use super::auth::get_valid_access_token;
use super::config::GDriveHostInfo;
use crate::provider::{MediaEntry, MediaSource};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use serde::Deserialize;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct ShortcutDetails {
    #[serde(rename = "targetId")]
    target_id: Option<String>,
    #[serde(rename = "targetMimeType")]
    target_mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    name: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
    size: Option<String>,
    #[serde(rename = "shortcutDetails")]
    shortcut_details: Option<ShortcutDetails>,
}

#[derive(Debug, Deserialize)]
struct FileListResponse {
    files: Vec<DriveFile>,
}

pub struct GDriveSource {
    pub host_info: GDriveHostInfo,
    pub client: reqwest::Client,
    root_path: String,
    // in-memory path-to-id cache
    path_cache: Arc<RwLock<HashMap<String, (String, bool)>>>,
}

impl GDriveSource {
    pub fn new(host_info: GDriveHostInfo) -> Self {
        let root_path = host_info.remote_path.clone();
        Self {
            host_info, //
            client: reqwest::Client::new(),
            root_path,
            path_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // resolve virtual path like folder1/file.mp4 to drive file/folder id
    async fn resolve_path_to_id(
        &self, //
        access_token: &str,
        path: &str,
    ) -> io::Result<(String, bool)> {
        // prepend self.root_path if path is empty or relative
        let full_path = match (self.root_path.trim_matches('/'), path.trim_matches('/')) {
            ("", "") => String::new(),
            ("", p) => p.to_string(),
            (r, "") => r.to_string(),
            (r, p) => format!("{}/{}", r, p),
        };

        if full_path.is_empty() {
            return Ok(("root".to_string(), true));
        }

        // fast path: read lock check
        {
            let cache = self.path_cache.read().await;
            if let Some(entry) = cache.get(&full_path) {
                return Ok(entry.clone());
            }
        }

        // resolve path level by level via API calls
        let segments: Vec<&str> = full_path.split('/').collect();
        let mut current_id = "root".to_string();
        let mut is_dir = true;
        let mut accumulated_path = String::new();

        for segment in segments {
            if !accumulated_path.is_empty() {
                accumulated_path.push('/');
            }
            accumulated_path.push_str(segment);

            // check if subpath is already cached
            {
                let cache = self.path_cache.read().await;
                if let Some((cached_id, cached_is_dir)) = cache.get(&accumulated_path) {
                    current_id = cached_id.clone();
                    is_dir = *cached_is_dir;
                    continue;
                }
            }

            // query drive api for folder/file id
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

            let (real_id, real_mime) = if file.mime_type == "application/vnd.google-apps.shortcut" {
                if let Some(ref details) = file.shortcut_details {
                    (
                        details.target_id.clone().unwrap_or_else(|| file.id.clone()),
                        details.target_mime_type.clone().unwrap_or_else(|| file.mime_type.clone()),
                    )
                } else {
                    (file.id.clone(), file.mime_type.clone())
                }
            } else {
                (file.id.clone(), file.mime_type.clone())
            };

            current_id = real_id;
            is_dir = real_mime == "application/vnd.google-apps.folder";

            // store resolved intermediate path in cache
            let mut cache = self.path_cache.write().await;
            cache.insert(accumulated_path.clone(), (current_id.clone(), is_dir));
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
            .client
            .get("https://www.googleapis.com/drive/v3/files")
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .query(&[
                ("q", query.as_str()),
                // request shortcutDetails field from drive api
                ("fields", "files(id, name, mimeType, size, shortcutDetails)"),
                ("pageSize", "1000"),
            ])
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .json()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let entries = res
            .files
            .into_iter()
            .map(|f| {
                // resolve if entry or shortcut target is a directory
                let is_folder = if f.mime_type == "application/vnd.google-apps.shortcut" {
                    f.shortcut_details
                        .as_ref()
                        .and_then(|d| d.target_mime_type.as_deref())
                        .map(|t| t == "application/vnd.google-apps.folder")
                        .unwrap_or(false)
                } else {
                    f.mime_type == "application/vnd.google-apps.folder"
                };

                let size = f.size.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                let entry_path = if path.trim_matches('/').is_empty() {
                    f.name.clone()
                } else {
                    format!("{}/{}", path.trim_matches('/'), f.name)
                };

                MediaEntry { name: f.name, path: entry_path, is_dir: is_folder, size }
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
