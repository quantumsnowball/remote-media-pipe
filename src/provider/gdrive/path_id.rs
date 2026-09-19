use super::file_list::FileListResponse;
use reqwest::header::AUTHORIZATION;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PathResolver {
    path_cache: Arc<RwLock<HashMap<String, (String, bool)>>>,
}

impl PathResolver {
    pub fn new() -> Self {
        Self {
            //
            path_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn resolve(
        &self,
        client: &reqwest::Client,
        access_token: &str,
        root_path: &str,
        path: &str,
    ) -> io::Result<(String, bool)> {
        // prepend root_path if path is empty or relative
        let full_path = match (root_path.trim_matches('/'), path.trim_matches('/')) {
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

            // query drive api for folder/file/shortcut id
            let query = format!(
                "'{}' in parents and name = '{}' and trashed = false",
                current_id,
                segment.replace('\'', "\\'")
            );

            let res: FileListResponse = client
                .get("https://www.googleapis.com/drive/v3/files")
                .header(AUTHORIZATION, format!("Bearer {}", access_token))
                .query(&[
                    ("q", query.as_str()), //
                    ("fields", "files(id, name, mimeType, shortcutDetails)"),
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

            // if entry is a shortcut, extract target_id and target_mime_type
            let (real_id, real_mime) = if file.mime_type == "application/vnd.google-apps.shortcut" {
                if let Some(ref details) = file.shortcut_details {
                    let tid = details.target_id.clone().unwrap_or_else(|| file.id.clone());
                    let tmime = details.target_mime_type.clone().unwrap_or_else(|| file.mime_type.clone());
                    (tid, tmime)
                } else {
                    (file.id.clone(), file.mime_type.clone())
                }
            } else {
                (file.id.clone(), file.mime_type.clone())
            };

            current_id = real_id;
            is_dir = real_mime == "application/vnd.google-apps.folder";

            // store resolved target path in cache
            let mut cache = self.path_cache.write().await;
            cache.insert(accumulated_path.clone(), (current_id.clone(), is_dir));
        }
        //
        Ok((current_id, is_dir))
    }
}
