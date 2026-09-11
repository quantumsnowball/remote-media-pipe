use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};

// ip whitelist middleware checking peer address against server bind target
pub async fn enforce_ip_whitelist(
    State(allowed_ip): State<IpAddr>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // allow only loopback traffic or exact target ip match, else gives a 403 forbidden
    let client_ip = peer_addr.ip();
    if client_ip == allowed_ip || client_ip.is_loopback() {
        Ok(next.run(request).await)
    } else {
        println!("[WARN] blocked request from unauthorized ip: {}", client_ip);
        Err(StatusCode::FORBIDDEN)
    }
}
