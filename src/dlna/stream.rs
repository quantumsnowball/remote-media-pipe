use crate::dlna::server::DlnaState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use std::sync::Arc;

pub async fn handle_stream(
    State(state): State<Arc<DlnaState>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let decoded_path = match urlencoding::decode(&path) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid path encoding").into_response(),
    };

    let range_header = headers //
        .get(header::RANGE)
        .and_then(|h| h.to_str().ok());

    match state.source.stream_file(&decoded_path, range_header).await {
        Ok(response) => response,
        Err(_) => (StatusCode::NOT_FOUND, "File not found or unreadable").into_response(),
    }
}
