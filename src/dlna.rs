use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const ROOT_DESC_TEMPLATE: &str = include_str!("../assets/rootDesc.xml");

// Static dummy folder listing for DLNA / UPnP players
const DUMMY_DIDL_RESPONSE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:BrowseResponse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <Result>&lt;DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/"&gt;
        &lt;container id="1" parentID="0" restricted="1"&gt;
          &lt;dc:title&gt;Dummy Movies&lt;/dc:title&gt;
          &lt;upnp:class&gt;object.container.storageFolder&lt;/upnp:class&gt;
        &lt;/container&gt;
        &lt;container id="2" parentID="0" restricted="1"&gt;
          &lt;dc:title&gt;Dummy Shows&lt;/dc:title&gt;
          &lt;upnp:class&gt;object.container.storageFolder&lt;/upnp:class&gt;
        &lt;/container&gt;
      &lt;/DIDL-Lite&gt;</Result>
      <NumberReturned>2</NumberReturned>
      <TotalMatches>2</TotalMatches>
      <UpdateID>1</UpdateID>
    </u:BrowseResponse>
  </s:Body>
</s:Envelope>"#;

pub struct Server {
    addr: SocketAddr,
    uuid: String,
}

impl Server {
    pub fn new(addr: SocketAddr, uuid: String) -> Self {
        Self { addr, uuid }
    }

    fn build_root_desc(&self) -> String {
        ROOT_DESC_TEMPLATE.replace("{{UUID}}", &self.uuid)
    }

    async fn handle_client(&self, mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 2048];
        let bytes_read = stream.read(&mut buf).await?;

        if bytes_read == 0 {
            return Ok(());
        }

        let request = String::from_utf8_lossy(&buf[..bytes_read]);

        // 1. Device Description Endpoint
        if request.contains("GET /rootDesc.xml") {
            let body = self.build_root_desc();
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: text/xml; charset=\"utf-8\"\r\n\
                Content-Length: {}\r\n\
                Connection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await?;
        }
        // 2. ContentDirectory Browse Endpoint (SOAP POST)
        else if request.contains("POST /ctl/ContentDirectory") || request.contains("Browse") {
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                Content-Type: text/xml; charset=\"utf-8\"\r\n\
                Content-Length: {}\r\n\
                Connection: close\r\n\r\n{}",
                DUMMY_DIDL_RESPONSE.len(),
                DUMMY_DIDL_RESPONSE
            );
            stream.write_all(response.as_bytes()).await?;
        }
        // 3. Fallback for unhandled endpoints
        else {
            let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }

        Ok(())
    }

    pub async fn listen(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;
        println!("[INFO] DLNA HTTP Server listening on http://{}", self.addr);

        loop {
            let (stream, _) = listener.accept().await?;
            let _ = self.handle_client(stream).await;
        }
    }
}
