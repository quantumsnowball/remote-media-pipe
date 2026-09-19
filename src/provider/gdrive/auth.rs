use super::config::GDriveHostInfo;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tracing::{debug, info};

// this data is saved at the cache dir and in ram
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedToken {
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
}

// the response
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

// global in-memory token cache keyed by profile_name
static RAM_TOKEN_CACHE: OnceLock<RwLock<HashMap<String, CachedToken>>> = OnceLock::new();

fn get_ram_cache() -> &'static RwLock<HashMap<String, CachedToken>> {
    RAM_TOKEN_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

// resolve token cache path: ~/.cache/remote-media-pipe/tokens/<profile>.json
fn get_cache_path(profile_name: &str) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let home = std::env::var("HOME")?;
    let dir = PathBuf::from(home) //
        .join(".cache")
        .join("remote-media-pipe")
        .join("tokens");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.json", profile_name)))
}

// read cached token from disk if valid with a 60 second buffer
fn load_disk_cached_token(profile_name: &str) -> Option<CachedToken> {
    let cache_path = get_cache_path(profile_name).ok()?;
    let content = fs::read_to_string(cache_path).ok()?;
    let cached: CachedToken = serde_json::from_str(&content).ok()?;
    let now = Utc::now();
    if cached.expires_at - Duration::seconds(60) > now {
        Some(cached) //
    } else {
        None
    }
}

// save freshly fetched token to cache file
fn save_cached_token(
    profile_name: &str,
    access_token: &str,
    expires_in: i64,
) -> Result<CachedToken, Box<dyn std::error::Error + Send + Sync>> {
    let cache_path = get_cache_path(profile_name)?;
    let expires_at = Utc::now() + Duration::seconds(expires_in);
    let cached = CachedToken { access_token: access_token.to_string(), expires_at };
    let json = serde_json::to_string_pretty(&cached)?;
    fs::write(cache_path, json)?;
    Ok(cached)
}

// exchange refresh token for fresh access token via google oauth endpoint
async fn fetch_fresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<(String, i64), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let res = client.post("https://oauth2.googleapis.com/token").form(&params).send().await?.error_for_status()?;

    let body: TokenResponse = res.json().await?;
    Ok((body.access_token, body.expires_in))
}

// get valid access token: try ram first, then disk cache, fallback to OAuth exchange
pub async fn get_valid_access_token(
    host_info: &GDriveHostInfo,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let now = Utc::now();
    let ram_cache = get_ram_cache();

    // fast path: check ram cache with non-blocking read lock
    {
        let cache = ram_cache.read().await;
        if let Some(token) = cache.get(&host_info.profile_name) {
            if token.expires_at - Duration::seconds(60) > now {
                debug!("using ram-cached access token for [{}]", host_info.profile_name);
                return Ok(token.access_token.clone());
            }
        }
    }

    // slow path: acquire write lock to populate ram from disk or oauth
    let mut cache = ram_cache.write().await;

    // double-check in case another async task refreshed it while waiting for write lock
    if let Some(token) = cache.get(&host_info.profile_name) {
        if token.expires_at - Duration::seconds(60) > now {
            return Ok(token.access_token.clone());
        }
    }

    // try loading from disk cache
    if let Some(disk_token) = load_disk_cached_token(&host_info.profile_name) {
        debug!("using disk-cached access token for [{}]", host_info.profile_name);
        cache.insert(host_info.profile_name.clone(), disk_token.clone());
        return Ok(disk_token.access_token);
    }

    // fetch fresh token if missing or expired on both ram and disk
    info!("fetching fresh access token for [{}]...", host_info.profile_name);
    let (access_token, expires_in) =
        fetch_fresh_access_token(&host_info.client_id, &host_info.client_secret, &host_info.refresh_token).await?;

    // write to disk and save to ram
    let cached = save_cached_token(&host_info.profile_name, &access_token, expires_in)?;
    cache.insert(host_info.profile_name.clone(), cached);

    Ok(access_token)
}
