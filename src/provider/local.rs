use super::{MediaEntry, MediaSource};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::{io, path::PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio_util::io::ReaderStream;

/// local filesystem provider implementing `MediaSource`
pub struct LocalSource {
    root_path: PathBuf,
}

impl LocalSource {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }

    /// resolves and prevents directory traversal outside `root_path`
    fn resolve_path(&self, req_path: &str) -> PathBuf {
        let clean = req_path.trim_start_matches('/');
        if clean.is_empty() {
            self.root_path.clone()
        } else {
            self.root_path.join(clean)
        }
    }

    /// parses HTTP Range headers e.g., "bytes=100-200" or "bytes=500-"
    fn parse_range(range_header: &str, file_size: u64) -> Option<(u64, u64)> {
        let range_str = range_header.strip_prefix("bytes=")?;
        let mut parts = range_str.split('-');
        let start: u64 = parts.next()?.parse().ok()?;
        let end: u64 = match parts.next() {
            Some(e) if !e.is_empty() => e.parse().ok()?,
            _ => file_size.saturating_sub(1),
        };

        if start <= end && start < file_size {
            Some((start, end.min(file_size - 1)))
        } else {
            None
        }
    }
}

#[async_trait]
impl MediaSource for LocalSource {
    async fn read_dir(
        &self,
        relative_path: &str
    ) -> io::Result<Vec<MediaEntry>> {
        let target_path = self.resolve_path(relative_path);
        let mut read_dir = tokio::fs::read_dir(&target_path).await?;
        let mut entries = Vec::new();

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata().await;
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let full_entry_path = entry.path();
            let rel_path = full_entry_path
                .strip_prefix(&self.root_path)
                .unwrap_or(&full_entry_path)
                .to_string_lossy()
                .to_string();

            entries.push(MediaEntry {
                name,
                path: format!("/{}", rel_path.trim_start_matches('/')),
                is_dir: file_type.is_dir(),
                size,
            });
        }
        Ok(entries)
    }

    async fn stream_file(
        &self,
        path: &str,
        range_header: Option<&str>,
    ) -> io::Result<Response<Body>> {
        let file_path = self.resolve_path(path);
        let mut file = File::open(&file_path).await?;
        let metadata = file.metadata().await?;
        let file_size = metadata.len();

        // partial Content (HTTP 206) - range request for seeking
        if let Some(range_raw) = range_header {
            if let Some((start, end)) = Self::parse_range(range_raw, file_size) {
                let chunk_length = end - start + 1;

                file.seek(SeekFrom::Start(start)).await?;
                let limited_reader = file.take(chunk_length);
                let stream = ReaderStream::new(limited_reader);

                let response = (
                    StatusCode::PARTIAL_CONTENT,
                    [
                    (header::CONTENT_TYPE, "video/mp4"),
                    (header::ACCEPT_RANGES, "bytes"),
                    (header::CONTENT_RANGE, &format!("bytes {}-{}/{}", start, end, file_size),),
                    (header::CONTENT_LENGTH, &chunk_length.to_string()),
                    ],
                    Body::from_stream(stream),
                )
                    .into_response();

                return Ok(response);
            }
        }

        // full Content (HTTP 200) - initial request without Range
        let stream = ReaderStream::new(file);
        let response = (
            StatusCode::OK,
            [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::ACCEPT_RANGES, "bytes"),
            (header::CONTENT_LENGTH, &file_size.to_string()),
            ],
            Body::from_stream(stream),
        )
            .into_response();

        Ok(response)
    }
}
