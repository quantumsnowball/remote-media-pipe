use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

mod dlna;
mod ssdp;

use dlna::DlnaServer;
use ssdp::Server as SsdpServer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Remote address/host
    remote: String,

    /// Target socket address (e.g., 192.168.1.81:7879)
    target: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let remote = &args.remote;
    let target = &args.target;

    println!("Remote : {remote}");
    println!("Target : {target}");

    let host = target.ip().to_string();
    let port = target.port();

    let ssdp_server = Arc::new(SsdpServer::new(&host, port));
    let dlna_server = DlnaServer::new(&ssdp_server.uuid(), &host, port);

    // 1. Spawn Axum HTTP Server Task
    let target_addr = *target;
    tokio::spawn(async move {
        if let Err(e) = dlna_server.run(target_addr).await {
            eprintln!("[ERROR] DLNA HTTP Server failed: {e}");
        }
    });

    // 2. Broadcast SSDP Announcement
    ssdp_server.advertise()?;

    // 3. Spawn SSDP Listener Thread
    let ssdp_clone = Arc::clone(&ssdp_server);
    let shutdown_signal = ssdp_server.shutdown_handle();

    let ssdp_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = ssdp_clone.listen() {
            eprintln!("[ERROR] SSDP server error: {e}");
        }
    });

    // 4. Handle Graceful Exit
    tokio::signal::ctrl_c().await?;
    println!("\nReceived Ctrl+C, shutting down...");

    shutdown_signal.store(false, Ordering::SeqCst);
    let _ = ssdp_handle.await;

    println!("Shutdown complete.");
    Ok(())
}
