use super::auth::get_valid_access_token;
use super::config::GDriveHostInfo;
use crate::provider::{MediaEntry, MediaSource};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use serde::Deserialize;
use std::io;

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
    size: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileListResponse {
    files: Vec<DriveFile>,
}

pub struct GDriveSource {
    pub host_info: GDriveHostInfo,
    pub client: reqwest::Client,
}

impl GDriveSource {
    pub fn new(host_info: GDriveHostInfo) -> Self {
        Self { host_info, client: reqwest::Client::new() }
    }

    // resolve virtual path likefolder1/file.mp4 to drive file/folder id
    async fn resolve_path_to_id(
        &self, //
        access_token: &str,
        path: &str,
    ) -> io::Result<(String, bool)> {
        let clean_path = path.trim_matches('/');
        if clean_path.is_empty() {
            // root folder target
            return Ok(("root".to_string(), true));
        }

        let segments: Vec<&str> = clean_path.split('/').collect();
        let mut current_id = "root".to_string();
        let mut is_dir = true;

        for segment in segments {
            let query = format!(
                "'{}' in parents and name = '{}' and trashed = false",
                current_id,
                segment.replace('\'', "\\'")
            );

            let res: FileListResponse = self
                .client
                .get("https://www.googleapis.com/drive/v3/files")
                .header(AUTHORIZATION, format!("Bearer {}", access_token))
                .query(&[
                    ("q", query.as_str()), //
                    ("fields", "files(id, name, mimeType)"),
                ])
                .send()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                .json()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let file = res.files.first().ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("path component '{}' not found", segment))
            })?;

            current_id = file.id.clone();
            is_dir = file.mime_type == "application/vnd.google-apps.folder";
        }

        Ok((current_id, is_dir))
    }
}

#[async_trait]
impl MediaSource for GDriveSource {
    async fn read_dir(&self, path: &str) -> io::Result<Vec<MediaEntry>> {
        let token = get_valid_access_token(&self.host_info)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))?;

        let (folder_id, is_dir) = self.resolve_path_to_id(&token, path).await?;
        if !is_dir {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is not a directory"));
        }

        let query = format!("'{}' in parents and trashed = false", folder_id);
        let res: FileListResponse = self
            .client //
            .get("https://www.googleapis.com/drive/v3/files")
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .query(&[
                ("q", query.as_str()), //
                ("fields", "files(id, name, mimeType, size)"),
                ("pageSize", "1000"),
            ])
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .json()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let entries = res
            .files //
            .into_iter()
            .map(|f| {
                let is_dir = f.mime_type == "application/vnd.google-apps.folder";
                let size = f.size.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let entry_path = if path.trim_matches('/').is_empty() {
                    f.name.clone()
                } else {
                    format!("{}/{}", path.trim_matches('/'), f.name)
                };
                MediaEntry { name: f.name, path: entry_path, is_dir, size }
            })
            .collect();

        Ok(entries)
    }

    async fn stream_file(&self, path: &str, range_header: Option<&str>) -> io::Result<Response<Body>> {
        let token = get_valid_access_token(&self.host_info)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))?;

        let (file_id, is_dir) = self.resolve_path_to_id(&token, path).await?;
        if is_dir {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "cannot stream a directory"));
        }

        let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);
        let mut req = self.client.get(&url).header(AUTHORIZATION, format!("Bearer {}", token));

        if let Some(range) = range_header {
            req = req.header(RANGE, range);
        }

        let res = req.send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let status = res.status();
        let mut builder = Response::builder().status(status);

        // pass through critical headers like content-type and content-range
        if let Some(ct) = res.headers().get(CONTENT_TYPE) {
            builder = builder.header(CONTENT_TYPE, ct.clone());
        }
        if let Some(cr) = res.headers().get(CONTENT_RANGE) {
            builder = builder.header(CONTENT_RANGE, cr.clone());
        }
        if let Some(cl) = res.headers().get(CONTENT_LENGTH) {
            builder = builder.header(CONTENT_LENGTH, cl.clone());
        }

        let body = Body::from_stream(res.bytes_stream());
        builder.body(body).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
