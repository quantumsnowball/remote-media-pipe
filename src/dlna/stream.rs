use crate::dlna::server::DlnaState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use std::sync::Arc;
use tracing::{debug, info};

pub async fn handle_stream(
    State(state): State<Arc<DlnaState>>,
    Path(path): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // path
    let decoded_path = match urlencoding::decode(&path) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid path encoding").into_response(),
    };

    // range from header
    let range_header = headers //
        .get(header::RANGE)
        .and_then(|h| h.to_str().ok());

    // logging
    let is_initial_start = match range_header {
        None => true,
        Some(r) => r.starts_with("bytes=0-"),
    };
    if is_initial_start {
        info!("[Play] {}", decoded_path);
    } else {
        debug!("[Seek] {} ({})", decoded_path, range_header.unwrap_or("none"));
    }

    // source stream
    match state.source.stream_file(&decoded_path, range_header).await {
        Ok(response) => response,
        Err(_) => (StatusCode::NOT_FOUND, "File not found or unreadable").into_response(),
    }
}
