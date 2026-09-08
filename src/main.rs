mod dlna;
mod ssdp;

use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser)]
#[command(version, about = "A zero-copy remote media streaming proxy")]
struct Cli {
    /// Remote source (e.g. qsc:DLNA/ or gdrive:folder_id)
    remote: String,

    /// Target listen address (e.g. 192.168.1.100:7879)
    target: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let remote = &args.remote;
    let target = &args.target;

    println!("Remote : {remote}");
    println!("Target : {target}");

    // Extract host IP and port from the target SocketAddr
    let host = target.ip().to_string();
    let port = target.port();

    // Instantiate both SSDP and DLNA servers sharing the same UUID
    let ssdp_server = ssdp::Server::new(host, port);
    let dlna_server = dlna::Server::new(*target, ssdp_server.uuid());

    // 1. Advertise SSDP presence on launch
    ssdp_server.advertise().await?;

    // 2. Run both SSDP and DLNA listening loops concurrently alongside Ctrl+C
    tokio::select! {
        res = ssdp_server.listen() => {
            if let Err(e) = res {
                eprintln!("[ERROR] SSDP Server error: {e}");
            }
        }
        res = dlna_server.listen() => {
            if let Err(e) = res {
                eprintln!("[ERROR] DLNA Server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[INFO] Received Ctrl+C, shutting down gracefully...");
        }
    }

    Ok(())
}
