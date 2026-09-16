use super::config::GDriveHostInfo;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// this data is saved at the cache dir
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

// resolve token cache path: ~/.cache/remote-media-pipe/tokens/<profile>.json
fn get_cache_path(profile_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    let dir = PathBuf::from(home) //
        .join(".cache")
        .join("remote-media-pipe")
        .join("tokens");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.json", profile_name)))
}

// read cached token if valid with a 60 second buffer
fn load_cached_token(profile_name: &str) -> Option<String> {
    let cache_path = get_cache_path(profile_name).ok()?;
    let content = fs::read_to_string(cache_path).ok()?;
    let cached: CachedToken = serde_json::from_str(&content).ok()?;
    // buffer 60s before actual expiration to avoid edge cases
    let now = Utc::now();
    if cached.expires_at - Duration::seconds(60) > now {
        Some(cached.access_token) //
    } else {
        None
    }
}

// save freshly fetched token to cache file
fn save_cached_token(
    profile_name: &str, //
    access_token: &str,
    expires_in: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = get_cache_path(profile_name)?;
    let expires_at = Utc::now() + Duration::seconds(expires_in);
    let cached = CachedToken { access_token: access_token.to_string(), expires_at };
    let json = serde_json::to_string_pretty(&cached)?;
    fs::write(cache_path, json)?;
    Ok(())
}

// exchange refresh token for fresh access token via google oauth endpoint
async fn fetch_fresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<(String, i64), Box<dyn std::error::Error>> {
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

// get valid access token: try cache first, fallback to OAuth exchange
pub async fn get_valid_access_token(host_info: &GDriveHostInfo) -> Result<String, Box<dyn std::error::Error>> {
    // try cached token
    if let Some(token) = load_cached_token(&host_info.profile_name) {
        println!("[INFO] using cached access token for [{}]", host_info.profile_name);
        return Ok(token);
    }

    // fetch fresh token if missing or expired
    println!("[INFO] fetching fresh access token for [{}]...", host_info.profile_name);
    let (access_token, expires_in) = fetch_fresh_access_token(
        &host_info.client_id, //
        &host_info.client_secret,
        &host_info.refresh_token,
    )
    .await?;

    // write to cache
    save_cached_token(&host_info.profile_name, &access_token, expires_in)?;
    //
    Ok(access_token.to_string())
}
