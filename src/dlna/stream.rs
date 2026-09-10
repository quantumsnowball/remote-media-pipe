use axum::{
    extract::Path,
    http::StatusCode,
    http::Request,
    body::Body,
    response::IntoResponse,
};
use tower_http::services::ServeFile;

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
