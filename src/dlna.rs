use axum::{
    Router,
    extract::State,
    http::header,
    response::IntoResponse,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

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
}

pub struct DlnaServer {
    state: Arc<DlnaState>,
}

impl DlnaServer {
    pub fn new(uuid: &str, remote: &str) -> Self {
        Self {
            state: Arc::new(DlnaState {
                uuid: uuid.to_string(),
                remote: remote.to_string(),
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

    if !body.contains("Browse") {
        return (
            [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
            XML_CD_SYSTEM_UPDATE.to_string(),
        )
            .into_response();
    }

    let didl_entries = if body.contains("<ObjectID>0") {
        format!("{}{}",
            format!(include_str!("../assets/didl_container.xml"), "dummy_dir", "dummy dir"),
            format!(include_str!("../assets/didl_item.xml"), "dummy_item", "dummy item.mp4")
        )
    } else {
        String::new()
    };

    let didl_content = format!(include_str!("../assets/didl_content.xml"), didl_entries);

    let response_xml = SOAP_BROWSE_WRAPPER.replace("{}", &didl_content);

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        response_xml,
    )
        .into_response()
}
