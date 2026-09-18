mod args;
mod dlna;
mod logging;
mod provider;
mod ssdp;
use args::parse_and_resolve_args;
use dlna::DlnaServer;
use logging::init_logging;
use ssdp::SsdpServer;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::{error, info};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // logging
    init_logging();

    // parse args
    let (source, target, allowed) = parse_and_resolve_args().await?;

    // create servers
    let uuid = Uuid::new_v4();
    let ssdp_server = Arc::new(SsdpServer::new(uuid, &target.ip(), target.port()));
    let dlna_server = DlnaServer::new(uuid, source, &target);

    // spawn Axum HTTP server task
    tokio::spawn(async move {
        if let Err(e) = dlna_server.run(target, allowed).await {
            error!("DLNA HTTP Server failed: {e}");
        }
    });

    // broadcast SSDP announcement
    ssdp_server.advertise()?;

    // spawn SSDP listener thread
    let ssdp_clone = Arc::clone(&ssdp_server);
    let shutdown_signal = ssdp_server.shutdown_handle();
    let ssdp_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = ssdp_clone.listen() {
            error!("SSDP server error: {e}");
        }
    });

    // handle graceful exit
    tokio::signal::ctrl_c().await?;
    info!("\nReceived Ctrl+C, shutting down...");
    shutdown_signal.store(false, Ordering::SeqCst);
    let _ = ssdp_handle.await;
    info!("Shutdown complete.");

    //
    Ok(())
}
