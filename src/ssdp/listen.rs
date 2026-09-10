use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::atomic::Ordering;
use std::time::Duration;
use super::SsdpServer;
use super::server::{SSDP_IP, SSDP_PORT, MEDIA_TYPE, SERVER_TYPE};

impl SsdpServer {
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
