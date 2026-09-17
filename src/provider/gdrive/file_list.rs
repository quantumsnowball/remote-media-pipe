use serde::Deserialize;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{debug, warn};

const DIR_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Deserialize, Clone)]
pub struct ShortcutDetails {
    #[serde(rename = "targetId")]
    pub target_id: Option<String>,
    #[serde(rename = "targetMimeType")]
    pub target_mime_type: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: Option<String>,
    #[serde(rename = "shortcutDetails")]
    pub shortcut_details: Option<ShortcutDetails>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FileListResponse {
    pub files: Vec<DriveFile>,
}

pub struct FileListCache {
    cache: Arc<RwLock<HashMap<String, (Instant, FileListResponse)>>>,
}

impl FileListCache {
    pub fn new() -> Self {
        Self { cache: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub async fn get_file_list_response(
        &self,
        client: &reqwest::Client,
        token: &str,
        folder_id: &str,
        path: &str,
    ) -> io::Result<FileListResponse> {
        // check query cache
        {
            let cache_read = self.cache.read().await;
            if let Some((fetched_at, response)) = cache_read.get(folder_id) {
                if fetched_at.elapsed() < DIR_CACHE_TTL {
                    debug!("Serving file list from cache for path={path}, id={folder_id}");
                    return Ok(response.clone());
                }
            }
        }

        // cache miss: fetch from google drive api
        let query = format!("'{}' in parents and trashed = false", folder_id);
        warn!("Query to google about path={path}, id={folder_id}");

        let res: FileListResponse = client
            .get("https://www.googleapis.com/drive/v3/files")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
            .query(&[
                ("q", query.as_str()),
                ("fields", "files(id, name, mimeType, size, shortcutDetails)"),
                ("pageSize", "1000"),
            ])
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            .json()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // insert into cache
        {
            let mut cache_write = self.cache.write().await;
            cache_write.insert(folder_id.to_string(), (Instant::now(), res.clone()));
        }

        //
        Ok(res)
    }
}
