use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

pub const SSDP_IP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
pub const SSDP_PORT: u16 = 1900;
pub const DOCUMENT: &str = "rootDesc.xml";
pub const MEDIA_TYPE: &str = "urn:schemas-upnp-org:device:MediaServer:1";
pub const SERVER_TYPE: &str = "Linux/3.4 DLNADOC/1.50 UPnP/1.0 DMS/1.0";

pub struct Server {
    pub(super) location: String,
    pub(super) uuid: Uuid,
    running: Arc<AtomicBool>,
}

impl Server {
    pub fn new(host: &str, port: u16) -> Self {
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

    /// Listen for discovery SSDP queries and respond to them
    pub fn listen(&self) -> io::Result<()> {
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        sock.set_reuse_address(true)?;
        #[cfg(unix)]
        sock.set_reuse_port(true)?;

        sock.set_multicast_loop_v4(true)?;

        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, SSDP_PORT);
        sock.bind(&bind_addr.into())?;
        sock.join_multicast_v4(&SSDP_IP, &Ipv4Addr::UNSPECIFIED)?;

        // Short timeout allows checking self.running periodically
        sock.set_read_timeout(Some(Duration::from_millis(500)))?;

        let std_sock: UdpSocket = sock.into();
        println!(
            "[INFO] Listening for SSDP M-SEARCH requests on port {}...",
            SSDP_PORT
        );

        let mut buf = [0u8; 1024];

        let response_msg = format!(
            "HTTP/1.1 200 OK\r\n\
            Cache-Control: max-age=1800\r\n\
            Ext: \r\n\
            Location: {}\r\n\
            Server: {}\r\n\
            St: {}\r\n\
            Usn: uuid:{}::{}\r\n\
            Content-Length: 0\r\n\r\n",
            self.location, SERVER_TYPE, MEDIA_TYPE, self.uuid, MEDIA_TYPE
        );

        while self.running.load(Ordering::SeqCst) {
            match std_sock.recv_from(&mut buf) {
                Ok((len, src_addr)) => {
                    let request = &buf[..len];
                    if request.starts_with(b"M-SEARCH") {
                        if let Err(e) = std_sock.send_to(response_msg.as_bytes(), src_addr) {
                            eprintln!("[ERROR] Failed to respond to {}: {}", src_addr, e);
                        } else {
                            println!("[DEBUG] sock.send_to(message, addr={})", src_addr);
                        }
                    }
                }
                Err(ref e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                    {
                        continue;
                    }
                Err(e) => {
                    eprintln!("[ERROR] SSDP socket error: {}", e);
                    return Err(e);
                    }
            }
        }

        println!("[INFO] SSDP listener stopped cleanly.");
        Ok(())
    }
}
