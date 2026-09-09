use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

mod ssdp;
use ssdp::Server;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Remote address/host
    remote: String,

    /// Target socket address (e.g., 192.168.1.50:8080)
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

    let server = Arc::new(Server::new(&host, port));

    // Send initial advertisement broadcast
    server.advertise()?;

    let server_clone = Arc::clone(&server);
    let shutdown_signal = server.shutdown_handle();

    // Spawn SSDP listener
    let handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = server_clone.listen() {
            eprintln!("[ERROR] SSDP server error: {e}");
        }
    });

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    println!("\nReceived Ctrl+C, shutting down SSDP server...");

    // Stop loop and join thread
    shutdown_signal.store(false, Ordering::SeqCst);
    let _ = handle.await;

    println!("Shutdown complete.");
    Ok(())
}
