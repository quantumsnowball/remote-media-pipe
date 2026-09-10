use super::server::{MEDIA_TYPE, SERVER_TYPE, SSDP_IP, SSDP_PORT};
use super::SsdpServer;
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{SocketAddr, SocketAddrV4, UdpSocket};

impl SsdpServer {
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
}
