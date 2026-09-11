use super::config::SshHostInfo;
use russh::keys::agent::client::AgentClient;
use russh::keys::{PrivateKeyWithHashAlg, decode_secret_key};
use russh::*;
use std::sync::Arc;

struct SftpClientHandler;

#[async_trait::async_trait]
impl client::Handler for SftpClientHandler {
    type Error = russh::Error;
}

pub async fn connect_ssh(dest: &SshHostInfo) -> Result<(), Box<dyn std::error::Error>> {
    println!("[INFO] connecting to {}:{}...", dest.addr, dest.port);

    let config = Arc::new(client::Config::default());
    let mut session = client::connect(config, (dest.addr.as_str(), dest.port), SftpClientHandler).await?;

    let mut authenticated = false;

    // 1. try ssh-agent first
    if let Ok(mut agent) = AgentClient::connect_env().await {
        if let Ok(identities) = agent.request_identities().await {
            for identity in identities {
                if session
                    .authenticate_publickey_with(
                        &dest.user, //
                        identity.public_key().into_owned(),
                        None,
                        &mut agent,
                    )
                    .await
                    .is_ok()
                {
                    println!("[INFO] authenticated via ssh-agent for user '{}'", dest.user);
                    authenticated = true;
                    break;
                }
            }
        }
    }

    // 2. fallback to identity file or default ~/.ssh keys
    if !authenticated {
        let key_path = match &dest.identity_file {
            Some(path) => path.clone(),
            None => {
                let home = dirs::home_dir().ok_or("could not find home directory")?;
                let ed25519 = home.join(".ssh/id_ed25519");
                let rsa = home.join(".ssh/id_rsa");

                if ed25519.exists() {
                    ed25519
                } else if rsa.exists() {
                    rsa
                } else {
                    return Err("no identity file specified and no default keys found in ~/.ssh/".into());
                }
            }
        };

        println!("[INFO] authenticating using key file: {}", key_path.display());
        let key_pair = decode_secret_key(&std::fs::read_to_string(&key_path)?, None)?;

        // Remove '?' because PrivateKeyWithHashAlg::new does not return a Result
        let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key_pair), None);
        session.authenticate_publickey(&dest.user, key_with_alg).await?;
    }

    // 3. prove connection is alive
    println!("[SUCCESS] ssh connection established and authenticated successfully!");

    Ok(())
}
