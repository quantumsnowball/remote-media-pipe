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
const XML_CD_SCPD: &str = include_str!("../assets/cd.xml");
const XML_CM_SCPD: &str = include_str!("../assets/cm.xml");
const XML_CM_SOAP_RESP: &str = include_str!("../assets/cm_soap_response.xml");
const XML_CD_SYSTEM_UPDATE: &str = include_str!("../assets/cd_system_update.xml");

const DIDL_ROOT: &str = include_str!("../assets/didl_root.xml");
const DIDL_MOVIES: &str = include_str!("../assets/didl_movies.xml");
const DIDL_MUSIC: &str = include_str!("../assets/didl_music.xml");

const SOAP_BROWSE_WRAPPER: &str = include_str!("../assets/_soap_browse_wrapper.xml");


#[derive(Clone)]
pub struct DlnaState {
    pub uuid: String,
}

pub struct DlnaServer {
    state: Arc<DlnaState>,
}

impl DlnaServer {
    pub fn new(uuid: &str) -> Self {
        Self {
            state: Arc::new(DlnaState {
                uuid: uuid.to_string(),
            }),
        }
    }

    pub async fn run(&self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/rootDesc.xml", get(handle_root_desc))
            .route("/cd.xml", get(handle_cd_scpd))
            .route("/cm.xml", get(handle_cm_scpd))
            .route("/ctl/ContentDirectory", post(handle_content_directory))
            .route("/ctl/ConnectionManager", post(handle_connection_manager))
            .with_state(self.state.clone());

        let listener = TcpListener::bind(addr).await?;
        println!("[INFO] Axum DLNA HTTP Server running on http://{}", addr);

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}

async fn handle_root_desc(State(state): State<Arc<DlnaState>>) -> impl IntoResponse {
    println!("[INFO] handle_root_desc");

    let xml = XML_ROOT_DESC.replace("{UUID}", &state.uuid);
    ([(header::CONTENT_TYPE, "text/xml; charset=utf-8")], xml)
}

async fn handle_cd_scpd() -> impl IntoResponse {
    println!("[INFO] handle_cd_scpd");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CD_SCPD,
    )
}

async fn handle_cm_scpd() -> impl IntoResponse {
    println!("[INFO] handle_cm_scpd");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CM_SCPD,
    )
}

async fn handle_connection_manager() -> impl IntoResponse {
    println!("[INFO] handle_connection_manager");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        XML_CM_SOAP_RESP,
    )
}

async fn handle_content_directory(body: String) -> impl IntoResponse {
    println!("[INFO] handle_content_directory");

    if !body.contains("Browse") {
        return (
            [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
            XML_CD_SYSTEM_UPDATE.to_string(),
        )
            .into_response();
    }

    let didl_content = if body.contains("ObjectID>1") || body.contains("ObjectID&gt;1") {
        DIDL_MOVIES
    } else if body.contains("ObjectID>2") || body.contains("ObjectID&gt;2") {
        DIDL_MUSIC
    } else {
        DIDL_ROOT
    };

    let response_xml = SOAP_BROWSE_WRAPPER.replace("{}", didl_content);

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        response_xml,
    )
        .into_response()
}
