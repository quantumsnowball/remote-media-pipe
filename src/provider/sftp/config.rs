use ssh2_config::{ParseRule, SshConfig};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Debug)]
pub struct SshHostInfo {
    pub addr: String,
    pub port: u16,
    pub user: String,
    pub identity_file: Option<PathBuf>,
    pub remote_path: String,
}

pub fn read_ssh_config(target: &str) -> Result<SshHostInfo, Box<dyn std::error::Error>> {
    // split target string like "s7:~/Movies"
    let parts: Vec<&str> = target.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("invalid format, expected profile:/path (e.g. s7:~/Movies)".into());
    }

    let profile_alias = parts[0];
    let remote_path = parts[1].to_string();

    // locate ~/.ssh/config
    let ssh_config_path = dirs::home_dir()
        .ok_or("could not find home directory")?
        .join(".ssh/config");

    println!("[DEBUG] reading ssh config from: {}", ssh_config_path.display());

    if !ssh_config_path.exists() {
        return Err(format!("ssh config file not found at {}", ssh_config_path.display()).into());
    }

    // parse ssh config file
    let file = File::open(&ssh_config_path)?;
    let mut reader = BufReader::new(file);
    let config = SshConfig::default().parse(&mut reader, ParseRule::STRICT)?;

    // query parameters for requested profile alias
    let params = config.query(profile_alias);

    let host = params.host_name.unwrap_or_else(|| profile_alias.to_string());
    let port = params.port.unwrap_or(22);
    let user = params.user.clone().unwrap_or_else(|| "unknown".to_string());
    let identity_file = params.identity_file.and_then(|files| files.first().cloned());

    let dest = SshHostInfo {
        addr: host.clone(),
        port,
        user: user.clone(),
        identity_file: identity_file.clone(),
        remote_path: remote_path.clone(),
    };

    Ok(dest)
}

pub fn print_ssh_config(target: &str) -> Result<SshHostInfo, Box<dyn std::error::Error>> {
    let c = read_ssh_config(&target)?;
    println!("--- SSH CONFIG RESOLUTION ---");
    println!(" host name     : {}", c.addr);
    println!(" port          : {}", c.port);
    println!(" user          : {}", c.user);
    println!(" identity file : {:?}", c.identity_file);
    println!(" remote path   : {}", c.remote_path);
    println!("-----------------------------");
    Ok(c)
}
