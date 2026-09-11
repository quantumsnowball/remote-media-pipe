use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};

// ip whitelist middleware checking peer address against server bind target
pub async fn enforce_ip_whitelist(
    State(whitelist): State<Vec<IpAddr>>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // allow loopback traffic or any whitelisted ip
    let src_ip = peer_addr.ip();
    if src_ip.is_loopback() || whitelist.contains(&src_ip) {
        Ok(next.run(request).await)
    } else {
        println!("[WARN] blocked request from unauthorized ip: {}", src_ip);
        Err(StatusCode::FORBIDDEN)
    }
}
