use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

const SSDP_IP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const SSDP_PORT: u16 = 1900;
const DOCUMENT: &str = "rootDesc.xml";
const MEDIA_TYPE: &str = "urn:schemas-upnp-org:device:MediaServer:1";
const SERVER_TYPE: &str = "Linux/3.4 DLNADOC/1.50 UPnP/1.0 DMS/1.0";

pub struct Server {
    location: String,
    uuid: Uuid,
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

    /// Returns a handle to signal shutdown
    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Auto advertise itself (referencing rclone)
    pub fn advertise(&self) -> io::Result<()> {
        let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        sock.set_multicast_ttl_v4(2)?;
        sock.set_multicast_loop_v4(true)?;

        let std_sock: UdpSocket = sock.into();
        let target_addr: SocketAddr = SocketAddr::V4(SocketAddrV4::new(SSDP_IP, SSDP_PORT));

        let notification_types = [
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            &format!("uuid:{}", self.uuid),
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            "upnp:rootdevice",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            MEDIA_TYPE,
        ];

        for nt in notification_types {
            let msg = format!(
                "NOTIFY * HTTP/1.1\r\n\
                HOST: {}:{}\r\n\
                NT: {}\r\n\
                NTS: ssdp:alive\r\n\
                SERVER: {}\r\n\
                USN: uuid:{}::{}\r\n\
                CACHE-CONTROL: max-age=1800\r\n\
                LOCATION: {}\r\n\r\n",
                SSDP_IP, SSDP_PORT, nt, SERVER_TYPE, self.uuid, MEDIA_TYPE, self.location
            );

            std_sock.send_to(msg.as_bytes(), target_addr)?;
        }

        println!("[INFO] Rust custom SSDP server started");
        Ok(())
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
