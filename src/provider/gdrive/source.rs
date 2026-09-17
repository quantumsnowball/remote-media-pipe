use super::auth::get_valid_access_token;
use super::config::GDriveHostInfo;
use super::file_list::FileListBrowser;
use super::path_id::PathResolver;
use crate::provider::{MediaEntry, MediaSource};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::Response;
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE};
use std::io;

pub struct GDriveSource {
    pub host_info: GDriveHostInfo,
    pub client: reqwest::Client,
    root_path: String,
    path_resolver: PathResolver,
    file_list_browser: FileListBrowser,
}

impl GDriveSource {
    pub fn new(host_info: GDriveHostInfo) -> Self {
        let root_path = host_info.remote_path.clone();
        Self {
            host_info, //
            client: reqwest::Client::new(),
            root_path,
            path_resolver: PathResolver::new(),
            file_list_browser: FileListBrowser::new(),
        }
    }
}

#[async_trait]
impl MediaSource for GDriveSource {
    async fn read_dir(&self, path: &str) -> io::Result<Vec<MediaEntry>> {
        // get a token
        let token = get_valid_access_token(&self.host_info)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))?;

        // resolve path to id
        let (folder_id, is_dir) = self.path_resolver.resolve(&self.client, &token, &self.root_path, &path).await?;

        // if not a directory, ends here
        if !is_dir {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "path is not a directory"));
        }

        // it is a dir, then ask file list browser to get a response
        let res = self.file_list_browser.get_file_list_response(&self.client, &token, &folder_id, path).await?;

        // parse the response and collect media entries
        let entries: Vec<MediaEntry> = res
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
                //
                MediaEntry { name: f.name, path: entry_path, is_dir: is_folder, size }
            })
            .collect();
        // return
        Ok(entries)
    }

    async fn stream_file(&self, path: &str, range_header: Option<&str>) -> io::Result<Response<Body>> {
        // get a token
        let token = get_valid_access_token(&self.host_info)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))?;

        // resolve path to id
        let (file_id, is_dir) = self.path_resolver.resolve(&self.client, &token, &self.root_path, &path).await?;

        // if is a directory, ends here
        if is_dir {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "cannot stream a directory"));
        }

        // it is a file, can be stream, so make a get request for it
        let url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id);
        let mut req = self.client.get(&url).header(AUTHORIZATION, format!("Bearer {}", token));
        // use range search
        if let Some(range) = range_header {
            req = req.header(RANGE, range);
        }
        // send
        let res = req.send().await.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        // get response
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
        // get body
        let body = Body::from_stream(res.bytes_stream());
        // return
        builder.body(body).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}
