use super::connection_manager::{handle_connection_manager, handle_ctl_connection_manager};
use super::content_directory::{handle_content_directory, handle_ctl_content_directory};
use super::root::handle_root_desc;
use super::stream::handle_stream;
use crate::provider::MediaSource;
use crate::provider::local::LocalSource;
use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct DlnaState {
    pub uuid: String,
    pub source: Arc<dyn MediaSource>,
    pub host: String,
}

pub struct DlnaServer {
    state: Arc<DlnaState>,
}

impl DlnaServer {
    pub fn new(
        uuid: &str, //
        remote: &str,
        target: &SocketAddr,
    ) -> Self {
        Self {
            state: Arc::new(DlnaState {
                uuid: uuid.to_string(),
                // TODO: based on the remote str, determine what impl source to use
                source: Arc::new(LocalSource::new(remote)),
                host: format!("{}:{}", target.ip(), target.port()),
            }),
        }
    }

    pub async fn run(
        &self, //
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // define all the routes
        let app = Router::new()
            .route("/rootDesc.xml", get(handle_root_desc))
            .route("/ContentDirectory.xml", get(handle_content_directory))
            .route("/ConnectionManager.xml", get(handle_connection_manager))
            .route("/ctl/ContentDirectory", post(handle_ctl_content_directory))
            .route("/ctl/ConnectionManager", post(handle_ctl_connection_manager))
            .route("/stream/{*path}", get(handle_stream))
            .with_state(self.state.clone());

        // bind addr
        let listener = TcpListener::bind(addr).await?;
        println!("[INFO] Axum DLNA HTTP Server running on http://{}", addr);

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
