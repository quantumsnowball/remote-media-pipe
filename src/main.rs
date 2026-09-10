mod dlna;
mod provider;
mod ssdp;
use clap::Parser;
use dlna::DlnaServer;
use ssdp::SsdpServer;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// remote string completely describe a resource
    remote: String,
    /// target socket address (e.g., 192.168.1.81:7879)
    target: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse args
    let args = Cli::parse();
    let remote = &args.remote;
    let target = &args.target;
    let host = target.ip();
    let port = target.port();

    // create servers
    let ssdp_server = Arc::new(SsdpServer::new(&host, port));
    let dlna_server = DlnaServer::new(&ssdp_server.uuid(), remote, target);

    // spawn Axum HTTP server task
    let target_addr = *target;
    tokio::spawn(async move {
        if let Err(e) = dlna_server.run(target_addr).await {
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
