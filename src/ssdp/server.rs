use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use uuid::Uuid;

pub const SSDP_IP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
pub const SSDP_PORT: u16 = 1900;
pub const DOCUMENT: &str = "rootDesc.xml";
pub const MEDIA_TYPE: &str = "urn:schemas-upnp-org:device:MediaServer:1";
pub const SERVER_TYPE: &str = "Linux/3.4 DLNADOC/1.50 UPnP/1.0 DMS/1.0";

pub struct SsdpServer {
    pub(super) location: String,
    pub(super) uuid: Uuid,
    pub(super) running: Arc<AtomicBool>,
}

impl SsdpServer {
    pub fn new(
        host: &IpAddr, //
        port: u16,
    ) -> Self {
        Self {
            location: format!("http://{}:{}/{}", host, port, DOCUMENT),
            uuid: Uuid::new_v4(),
            running: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Expose UUID for DLNA HTTP Server initialization
    pub fn uuid(&self) -> String {
        self.uuid.to_string()
    }

    /// Returns a handle to signal shutdown
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }
}
