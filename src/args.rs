use super::provider::MediaSource;
use super::provider::gdrive::{GDriveSource, print_gdrive_config};
use super::provider::local::LocalSource;
use super::provider::sftp::{SftpSource, connect_sftp, print_ssh_config};
use clap::{Parser, Subcommand};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "remote-media-pipe")]
#[command(about = "A non-root remote media streaming pipe")]
pub struct Cli {
    #[command(subcommand)]
    pub provider: ProviderSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ProviderSubcommand {
    /// local directory path (e.g. ~/Movies or /data/video)
    Local {
        source: PathBuf,
        target: SocketAddr,
        #[arg(long = "allow", value_name = "IP address", num_args = 1..)]
        allowed: Vec<IpAddr>,
    },
    /// remote target (e.g. s7:~/Movies or s7:/var/media)
    Sftp {
        source: String,
        target: SocketAddr,
        #[arg(long = "allow", value_name = "IP address", num_args = 1..)]
        allowed: Vec<IpAddr>,
    },
    /// remote folder path or ID (e.g. qsc:DLNA/)
    Gdrive {
        source: String,
        target: SocketAddr,
        #[arg(long = "allow", value_name = "IP address", num_args = 1..)]
        allowed: Vec<IpAddr>,
    },
}

pub type ResolvedArgs = (
    Arc<dyn MediaSource>, //
    SocketAddr,
    Vec<IpAddr>,
);
pub async fn parse_and_resolve_args() -> Result<ResolvedArgs, Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    //
    let (source, target, allowed): ResolvedArgs = match cli.provider {
        ProviderSubcommand::Local { source, target, allowed } => {
            (
                Arc::new(LocalSource::new(source)), //
                target,
                allowed,
            )
        }
        ProviderSubcommand::Sftp { source, target, allowed } => {
            let host = print_ssh_config(&source)?;
            let session = connect_sftp(&host).await?;
            (
                Arc::new(SftpSource::new(session, host.remote_path)), //
                target,
                allowed,
            )
        }
        ProviderSubcommand::Gdrive { source, target, allowed } => {
            let host = print_gdrive_config(&source)?;
            (
                Arc::new(GDriveSource::new(host)), //
                target,
                allowed,
            )
        }
    };
    //
    Ok((source, target, allowed))
}
