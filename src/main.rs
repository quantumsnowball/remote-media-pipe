mod dlna;
mod provider;
mod ssdp;
use clap::Parser;
use dlna::DlnaServer;
use ssdp::SsdpServer;
use uuid::Uuid;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use clap::{Subcommand};
use std::path::PathBuf;
use provider::MediaSource;
use provider::local::LocalSource;
use provider::sftp::read_ssh_config;

#[derive(Parser, Debug)]
#[command(name = "remote-media-pipe")]
#[command(about = "A non-root remote media streaming pipe")]
pub struct Cli {
    #[command(subcommand)]
    pub provider: ProviderSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum ProviderSubcommand {
    /// Local directory path (e.g. ~/Movies or /data/video)
    Local {
        source: PathBuf,
        target: SocketAddr,
    },
    /// Remote target (e.g. s7:~/Movies or s7:/var/media)
    Sftp {
        source: String,
        target: SocketAddr,
    },
    /// Remote folder path or ID (e.g. qsc:DLNA/)
    Gdrive {
        source: String,
        target: SocketAddr,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse args
    let cli = Cli::parse();
    let (source, target): (Arc<dyn MediaSource>, SocketAddr) = match cli.provider {
        ProviderSubcommand::Local { source, target } => {
            (Arc::new(LocalSource::new(source)), target)
        }
        ProviderSubcommand::Sftp { source, target } => {
            let c = read_ssh_config(&source)?;
            println!("--- SSH CONFIG RESOLUTION ---");
            println!(" host name     : {}", c.addr);
            println!(" port          : {}", c.port);
            println!(" user          : {}", c.user);
            println!(" identity file : {:?}", c.identity_file);
            println!(" remote path   : {}", c.remote_path);
            println!("-----------------------------");
            println!(" target        : {}", target);
            todo!("Implement sftp");
        }
        ProviderSubcommand::Gdrive { source, target } => {
            println!("[INFO] args: {}, {}", source, target);
            todo!("Implement gdrive");
        }
    };
    println!("[INFO] Starting server bound to {}", target);

    // create servers
    let uuid = Uuid::new_v4();
    let ssdp_server = Arc::new(SsdpServer::new(uuid, &target.ip(), target.port()));
    let dlna_server = DlnaServer::new(uuid, source, &target);

    // spawn Axum HTTP server task
    tokio::spawn(async move {
        if let Err(e) = dlna_server.run(target).await {
            eprintln!("[ERROR] DLNA HTTP Server failed: {e}");
        }
    });

    // broadcast SSDP announcement
    ssdp_server.advertise()?;

    // spawn SSDP listener thread
    let ssdp_clone = Arc::clone(&ssdp_server);
    let shutdown_signal = ssdp_server.shutdown_handle();
    let ssdp_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = ssdp_clone.listen() {
            eprintln!("[ERROR] SSDP server error: {e}");
        }
    });

    // handle graceful exit
    tokio::signal::ctrl_c().await?;
    println!("\nReceived Ctrl+C, shutting down...");
    shutdown_signal.store(false, Ordering::SeqCst);
    let _ = ssdp_handle.await;
    println!("Shutdown complete.");

    //
    Ok(())
}
