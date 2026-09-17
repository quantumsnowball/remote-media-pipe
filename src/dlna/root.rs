use super::server::DlnaState;
use axum::{extract::State, http::header, response::IntoResponse};
use std::sync::Arc;
use tracing::info;

const XML_ROOT_DESC: &str = include_str!("../../assets/rootDesc.xml");

pub async fn handle_root_desc(
    //
    State(state): State<Arc<DlnaState>>,
    //
) -> impl IntoResponse {
    info!("handle_root_desc");

    let xml = XML_ROOT_DESC.replace("{UUID}", &state.uuid.to_string());
    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
        xml,
    )
}
