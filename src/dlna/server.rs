use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    http::Request,
    body::Body,
    response::IntoResponse,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::services::ServeFile;
use super::root::handle_root_desc;
use super::connection_manager::{handle_connection_manager, handle_ctl_connection_manager};
use super::content_directory::{handle_content_directory, handle_ctl_content_directory};


#[derive(Clone)]
pub struct DlnaState {
    pub uuid: String,
    pub remote: String,
    pub host: String,
}

pub struct DlnaServer {
    state: Arc<DlnaState>,
}

impl DlnaServer {
    pub fn new(uuid: &str, remote: &str, target: &SocketAddr) -> Self {
        Self {
            state: Arc::new(DlnaState {
                uuid: uuid.to_string(),
                remote: remote.to_string(),
                host: format!("{}:{}", target.ip(), target.port())
            }),
        }
    }

    pub async fn run(&self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/rootDesc.xml", get(handle_root_desc))
            .route("/ContentDirectory.xml", get(handle_content_directory))
            .route("/ConnectionManager.xml", get(handle_connection_manager))
            .route("/ctl/ContentDirectory", post(handle_ctl_content_directory))
            .route("/ctl/ConnectionManager", post(handle_ctl_connection_manager))
            .route("/stream/{*path}", get(handle_stream))
            .with_state(self.state.clone());

        let listener = TcpListener::bind(addr).await?;
        println!("[INFO] Axum DLNA HTTP Server running on http://{}", addr);

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        ).await?;

        Ok(())
    }
}

pub async fn handle_stream(
    Path(path): Path<String>,
    req: Request<Body>,
) -> impl IntoResponse {
    // Axum strips the leading slash from wildcard matches, so re-add it
    let full_path = format!("/{}", path);
    println!("[INFO] Serving file: {}", full_path);

    match ServeFile::new(&full_path).try_call(req).await {
        Ok(response) => response.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to stream file").into_response(),
    }
}

