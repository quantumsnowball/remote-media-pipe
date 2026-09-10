use axum::{
    Router,
    extract::{Query, State},
    http::{header, StatusCode},
    http::Request,
    body::Body,
    response::IntoResponse,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use serde::Deserialize;
use tower_http::services::ServeFile;

// Compile-time static assets
const XML_ROOT_DESC: &str = include_str!("../assets/rootDesc.xml");
const XML_CD_SCPD: &str = include_str!("../assets/ConnectionManager.xml");
const XML_CM_SCPD: &str = include_str!("../assets/ConnectionManager.xml");
const XML_CM_SOAP_RESP: &str = include_str!("../assets/cm_soap_response.xml");
const XML_CD_SYSTEM_UPDATE: &str = include_str!("../assets/cd_system_update.xml");

const SOAP_BROWSE_WRAPPER: &str = include_str!("../assets/soap_browse_wrapper.xml");


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

async fn handle_root_desc(State(state): State<Arc<DlnaState>>) -> impl IntoResponse {
    println!("[INFO] handle_root_desc");

    let xml = XML_ROOT_DESC.replace("{UUID}", &state.uuid);
    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml
    )
}

async fn handle_content_directory() -> impl IntoResponse {
    println!("[INFO] handle_content_directory");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CD_SCPD,
    )
}

async fn handle_connection_manager() -> impl IntoResponse {
    println!("[INFO] handle_connection_manager");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CM_SCPD,
    )
}

async fn handle_ctl_connection_manager() -> impl IntoResponse {
    println!("[INFO] handle_ctl_connection_manager");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CM_SOAP_RESP,
    )
}

async fn handle_ctl_content_directory(State(state): State<Arc<DlnaState>>, body: String) -> impl IntoResponse {
    println!("[INFO] handle_ctl_content_directory");

    // end here if user doesn't not click on a directory
    if !body.contains("Browse") {
        return (
            [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
            XML_CD_SYSTEM_UPDATE.to_string(),
        )
            .into_response();
    }

    // user click on a directory, the object_id should be the directory path
    let object_id = ["<ObjectID>", "&lt;ObjectID&gt;"]
        .iter()
        .find_map(|&pattern| {
            let rest = &body[body.find(pattern)? + pattern.len()..];
            let end = rest.find('<').or_else(|| rest.find("&lt;"))?;
            let id = rest[..end].trim();
            (!id.is_empty()).then(|| id.to_string())
        }).unwrap_or_else(|| "0".to_string());

    // determine the target_dir from object_id
    let target_dir = if object_id == "0" {std::path::PathBuf::from(&state.remote)} else {std::path::PathBuf::from(&object_id)};

    // list the target_dir then generate the didl_entries
    let mut didl_entries = String::new();
    if let Ok(mut read_dir) = tokio::fs::read_dir(&target_dir).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            let name = entry.file_name().to_string_lossy().replace('&', ".");
            let path = entry.path().to_string_lossy().to_string();
            if file_type.is_dir() {
                // inject directory template
                didl_entries.push_str(&format!(include_str!("../assets/didl_container.xml"), path, name));
            } else if file_type.is_file() {
                // inject video file template
                let stream_url = format!("http://{}/stream?path={}", state.host, urlencoding::encode(&path));
                didl_entries.push_str(&format!(include_str!("../assets/didl_item.xml"), path, name, stream_url));
            }
        }
    }

    // wrap didl_entries to didl_content
    let didl_content = format!(include_str!("../assets/didl_content.xml"), didl_entries);

    // wrap the didl_content with the soap browser wrapper
    let response_xml = SOAP_BROWSE_WRAPPER.replace("{}", &didl_content);

    // reply
    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        response_xml,
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct StreamQuery {
    pub path: String,
}

pub async fn handle_stream(
    Query(query): Query<StreamQuery>,
    req: Request<Body>,
) -> impl IntoResponse {
    // serve the query path directly using tower-http
    println!("Serving file {}", &query.path);
    match ServeFile::new(&query.path).try_call(req).await {
        Ok(response) => response.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to stream file").into_response(),
    }
}
