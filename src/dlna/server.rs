use super::connection_manager::{handle_connection_manager, handle_ctl_connection_manager};
use super::content_directory::{handle_content_directory, handle_ctl_content_directory};
use super::filter::enforce_ip_whitelist;
use super::root::handle_root_desc;
use super::stream::handle_stream;
use crate::provider::MediaSource;
use axum::{
    Router, middleware,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone)]
pub struct DlnaState {
    pub uuid: Uuid,
    pub source: Arc<dyn MediaSource>,
    pub host: String,
}

pub struct DlnaServer {
    state: Arc<DlnaState>,
}

impl DlnaServer {
    pub fn new(
        uuid: Uuid, //
        source: Arc<dyn MediaSource>,
        target: &SocketAddr,
    ) -> Self {
        Self {
            state: Arc::new(DlnaState {
                uuid,
                source,
                host: format!("{}:{}", target.ip(), target.port()),
            }),
        }
    }

    pub async fn run(
        &self, //
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // allowed
        let allowed_ip = addr.ip();

        // define all the routes
        let app = Router::new()
            .route("/rootDesc.xml", get(handle_root_desc))
            .route("/ContentDirectory.xml", get(handle_content_directory))
            .route("/ConnectionManager.xml", get(handle_connection_manager))
            .route("/ctl/ContentDirectory", post(handle_ctl_content_directory))
            .route("/ctl/ConnectionManager", post(handle_ctl_connection_manager))
            .route("/stream/{*path}", get(handle_stream))
            .with_state(self.state.clone())
            .layer(middleware::from_fn_with_state(allowed_ip, enforce_ip_whitelist));

        // bind addr
        let listener = TcpListener::bind(addr).await?;
        println!("[INFO] Axum DLNA HTTP Server running on http://{}", addr);
        println!("[INFO] IP whitelist active: restricting access to {}", allowed_ip);

        // serve
        axum::serve(
            listener, //
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        //
        Ok(())
    }
}
