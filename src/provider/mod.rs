pub mod gdrive;
pub mod local;
pub mod sftp;

use async_trait::async_trait;
use axum::{body::Body, response::Response};
use std::io;

#[derive(Debug, Clone)]
pub struct MediaEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

#[async_trait]
pub trait MediaSource: Send + Sync {

    async fn read_dir(
        &self,
        path: &str
    ) -> io::Result<Vec<MediaEntry>>;

    async fn stream_file(
        &self,
        path: &str,
        range_header: Option<&str>,
    ) -> io::Result<Response<Body>>;
}
