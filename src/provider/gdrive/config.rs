use ini::Ini;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

// internal helper struct to deserialize token json field from ini
#[derive(Debug, Deserialize)]
struct TokenJson {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expiry: String,
}

// result struct for further process
#[derive(Debug, Clone)]
pub struct GDriveHostInfo {
    pub profile_name: String,
    pub remote_path: String,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: String,
    pub expiry: String,
}

// resolve path to ~/.config/remote-media-pipe/gdrive.conf
fn parse_gdrive_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    let path = PathBuf::from(home).join(".config/remote-media-pipe/gdrive.conf");
    Ok(path)
}

// split rclone style path into profile name and remote path (e.g. 'qsc:DLNA/' -> ('qsc', 'DLNA/'))
fn parse_remote_path(source: &str) -> Result<(&str, &str), Box<dyn std::error::Error>> {
    let parts: Vec<&str> = source.splitn(2, ':').collect();
    if parts.len() < 2 || parts[0].is_empty() {
        return Err(format!("invalid source format '{}'. expected 'profile:path' (e.g. 'qsc:DLNA/')", source).into());
    }
    Ok((parts[0], parts[1]))
}

// load ini config from file path and parse section matched by source string
fn read_gdrive_config_from_file<P: AsRef<Path>>(
    config_path: P,
    source: &str,
) -> Result<GDriveHostInfo, Box<dyn std::error::Error>> {
    let (profile_name, remote_path) = parse_remote_path(source)?;
    let config_str = fs::read_to_string(config_path)?;
    let config_ini = Ini::load_from_str(&config_str)?;
    let config_section = config_ini
        .section(Some(profile_name)) //
        .ok_or_else(|| format!("profile [{}] not found in config", profile_name))?;
    let client_id = config_section
        .get("client_id") //
        .ok_or_else(|| format!("client_id missing in profile [{}]", profile_name))?
        .to_string();
    let client_secret = config_section
        .get("client_secret") //
        .ok_or_else(|| format!("client_secret missing in profile [{}]", profile_name))?
        .to_string();
    let token_str = config_section
        .get("token") //
        .ok_or_else(|| format!("token field missing in profile [{}]", profile_name))?;
    let t: TokenJson = serde_json::from_str(token_str)?;
    Ok(GDriveHostInfo {
        profile_name: profile_name.to_string(),
        remote_path: remote_path.to_string(),
        client_id,
        client_secret,
        access_token: t.access_token,
        token_type: t.token_type,
        refresh_token: t.refresh_token,
        expiry: t.expiry,
    })
}

// read gdrive config using full source string like 'qsc:DLNA/'
pub fn read_gdrive_config(source: &str) -> Result<GDriveHostInfo, Box<dyn std::error::Error>> {
    let config_path = parse_gdrive_config_path()?;
    read_gdrive_config_from_file(&config_path, source)
}

pub fn print_gdrive_config(source: &str) -> Result<GDriveHostInfo, Box<dyn std::error::Error>> {
    let c = read_gdrive_config(&source)?;
    println!("\n=== GDrive Host Info ===");
    println!("profile_name: {}", c.profile_name);
    println!("remote_path: {}", c.remote_path);
    println!("client_id: {}", c.client_id);
    println!("client_secret: {}", c.client_secret);
    println!("access_token: {}", c.access_token);
    println!("token_type: {}", c.token_type);
    println!("refresh_token: {}", c.refresh_token);
    println!("expiry: {}", c.expiry);
    Ok(c)
}
