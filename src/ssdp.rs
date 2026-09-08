use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;

pub struct Server {
    host: String,
    port: u16,
    multicast_addr: SocketAddr,
}

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            multicast_addr: SocketAddr::new(Ipv4Addr::new(239, 255, 255, 250).into(), 1900),
        }
    }

    /// Advertise itself on launch (sends SSDP NOTIFY via multicast)
    pub async fn advertise(&self, socket: &UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
        let notify_msg = format!(
            "NOTIFY * HTTP/1.1\r\n\
            HOST: 239.255.255.250:1900\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: http://{}:{}/description.xml\r\n\
            NT: urn:schemas-upnp-org:device:MediaServer:1\r\n\
            NTS: ssdp:alive\r\n\
            USN: uuid:remote-media-pipe-01::urn:schemas-upnp-org:device:MediaServer:1\r\n\r\n",
            self.host, self.port
        );

        socket
            .send_to(notify_msg.as_bytes(), self.multicast_addr)
            .await?;
        println!("SSDP advertisement broadcasted.");
        Ok(())
    }

    /// Keep listening forever until interrupted
    pub async fn listen(&self, socket: &UdpSocket) -> Result<(), Box<dyn std::error::Error>> {
        println!("SSDP listening for M-SEARCH queries...");
        let mut buf = [0u8; 1024];

        loop {
            let (len, src) = socket.recv_from(&mut buf).await?;
            let request = String::from_utf8_lossy(&buf[..len]);

            if request.contains("M-SEARCH") {
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                    CACHE-CONTROL: max-age=1800\r\n\
                    LOCATION: http://{}:{}/description.xml\r\n\
                    ST: urn:schemas-upnp-org:device:MediaServer:1\r\n\
                    USN: uuid:remote-media-pipe-01::urn:schemas-upnp-org:device:MediaServer:1\r\n\r\n",
                    self.host, self.port
                );

                socket.send_to(response.as_bytes(), src).await?;
            }
        }
    }

    /// Convenience wrapper to bind socket, advertise, and listen
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind("0.0.0.0:1900").await?;
        socket.join_multicast_v4(Ipv4Addr::new(239, 255, 255, 250), Ipv4Addr::UNSPECIFIED)?;

        // 1. Advertise on launch
        self.advertise(&socket).await?;

        // 2. Keep listening forever
        self.listen(&socket).await?;

        Ok(())
    }
}
