use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use uuid::Uuid;

pub struct Server {
    ssdp_ip: Ipv4Addr,
    ssdp_port: u16,
    media_type: &'static str,
    server_type: &'static str,
    uuid: Uuid,
    location: String,
}

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        let ssdp_ip = Ipv4Addr::new(239, 255, 255, 250);
        let ssdp_port = 1900;
        let document = "rootDesc.xml";
        let location = format!("http://{host}:{port}/{document}");

        Self {
            ssdp_ip,
            ssdp_port,
            media_type: "urn:schemas-upnp-org:device:MediaServer:1",
            server_type: "Linux/3.4 DLNADOC/1.50 UPnP/1.0 DMS/1.0",
            uuid: Uuid::new_v4(),
            location,
        }
    }

    pub fn uuid(&self) -> String {
        self.uuid.to_string()
    }

    /// Construct a NOTIFY advertisement message for a specific NT header value
    fn build_notify_message(&self, nt: &str) -> Vec<u8> {
        format!(
            "NOTIFY * HTTP/1.1\r\n\
            HOST: {}:{}\r\n\
            NT: {}\r\n\
            NTS: ssdp:alive\r\n\
            SERVER: {}\r\n\
            USN: uuid:{}::{}\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: {}\r\n\r\n",
            self.ssdp_ip,
            self.ssdp_port,
            nt,
            self.server_type,
            self.uuid,
            self.media_type,
            self.location
        )
        .into_bytes()
    }

    /// Construct the M-SEARCH 200 OK response message
    fn build_search_response(&self) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\n\
            Cache-Control: max-age=1800\r\n\
            Ext: \r\n\
            Location: {}\r\n\
            Server: {}\r\n\
            St: {}\r\n\
            Usn: uuid:{}::{}\r\n\
            Content-Length: 0\r\n\r\n",
            self.location, self.server_type, self.media_type, self.uuid, self.media_type
        )
        .into_bytes()
    }

    /// Broadcast NOTIFY messages on launch (matching rclone NT target fields)
    pub async fn advertise(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Ephemeral UDP socket for broadcasting
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        sock.set_multicast_ttl_v4(2)?;

        let target_addr: SocketAddr = (self.ssdp_ip, self.ssdp_port).into();

        let nt_targets = [
            "urn:microsoft.com:service:X_MS_MediaReceiverRegistrar:1",
            &format!("uuid:{}", self.uuid),
            "urn:schemas-upnp-org:service:ConnectionManager:1",
            "upnp:rootdevice",
            "urn:schemas-upnp-org:service:ContentDirectory:1",
            "urn:schemas-upnp-org:device:MediaServer:1",
        ];

        for nt in nt_targets {
            let msg = self.build_notify_message(nt);
            sock.send_to(&msg, target_addr).await?;
        }

        println!("[INFO] Rust custom SSDP server started (advertised)");
        Ok(())
    }

    /// Listen on port 1900 for M-SEARCH queries and respond
    pub async fn listen(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Bind UDP socket to port 1900
        let sock = UdpSocket::bind(("0.0.0.0", self.ssdp_port)).await?;

        // Join SSDP multicast group
        sock.join_multicast_v4(self.ssdp_ip, Ipv4Addr::UNSPECIFIED)?;
        sock.set_multicast_loop_v4(true)?;

        println!(
            "[INFO] Listening for SSDP M-SEARCH requests on port {}...",
            self.ssdp_port
        );

        let mut buf = [0u8; 1024];

        loop {
            let (len, src_addr) = sock.recv_from(&mut buf).await?;

            // Match Python's data.startswith(b"M-SEARCH")
            if buf[..len].starts_with(b"M-SEARCH") {
                let response = self.build_search_response();
                sock.send_to(&response, src_addr).await?;
                println!("[DEBUG] Responded to M-SEARCH from {src_addr}");
            }
        }
    }
}
