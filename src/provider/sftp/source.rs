use crate::provider::{MediaEntry, MediaSource};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use russh_sftp::client::SftpSession;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncSeekExt;
use tokio_util::io::ReaderStream;

pub struct SftpSource {
    sftp: Arc<SftpSession>,
    root_path: PathBuf,
}

impl SftpSource {
    pub fn new(sftp: SftpSession, root_path: impl Into<PathBuf>) -> Self {
        Self {
            sftp: Arc::new(sftp),
            root_path: root_path.into(),
        }
    }

    fn resolve_path(&self, req_path: &str) -> PathBuf {
        let clean = req_path.trim_start_matches('/');
        if clean.is_empty() {
            self.root_path.clone()
        } else {
            self.root_path.join(clean)
        }
    }

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
impl MediaSource for SftpSource {
    async fn read_dir(&self, relative_path: &str) -> io::Result<Vec<MediaEntry>> {
        let target_path = self.resolve_path(relative_path);
        let read_dir = self
            .sftp
            .read_dir(target_path.to_string_lossy().as_ref())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut entries = Vec::new();

        for file in read_dir {
            let name = file.file_name();

            if name.starts_with('.') {
                continue;
            }

            let is_dir = file.file_type().is_dir();
            let size = file.metadata().size.unwrap_or(0);

            let rel_path = target_path
                .join(&name)
                .strip_prefix(&self.root_path)
                .unwrap_or(&target_path.join(&name))
                .to_string_lossy()
                .to_string();

            entries.push(MediaEntry {
                name,
                path: format!("/{}", rel_path.trim_start_matches('/')),
                is_dir,
                size,
            });
        }

        Ok(entries)
    }

    async fn stream_file(&self, path: &str, range_header: Option<&str>) -> io::Result<Response<Body>> {
        let file_path = self.resolve_path(path);
        let mut file = self
            .sftp
            .open(file_path.to_string_lossy().as_ref())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let metadata = file
            .metadata()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let file_size = metadata.size.unwrap_or(0);

        if let Some(range_raw) = range_header {
            if let Some((start, end)) = Self::parse_range(range_raw, file_size) {
                let chunk_length = end - start + 1;

                file.seek(std::io::SeekFrom::Start(start)).await?;
                let limited_reader = tokio::io::AsyncReadExt::take(file, chunk_length);
                let stream = ReaderStream::new(limited_reader);

                return Ok((
                    StatusCode::PARTIAL_CONTENT,
                    [
                        (header::CONTENT_TYPE, "video/mp4"),
                        (header::ACCEPT_RANGES, "bytes"),
                        (header::CONTENT_RANGE, &format!("bytes {}-{}/{}", start, end, file_size)),
                        (header::CONTENT_LENGTH, &chunk_length.to_string()),
                    ],
                    Body::from_stream(stream),
                )
                    .into_response());
            }
        }

        let stream = ReaderStream::new(file);
        Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "video/mp4"),
                (header::ACCEPT_RANGES, "bytes"),
                (header::CONTENT_LENGTH, &file_size.to_string()),
            ],
            Body::from_stream(stream),
        )
            .into_response())
    }
}
