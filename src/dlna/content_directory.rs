use super::server::DlnaState;
use axum::{extract::State, http::header, response::IntoResponse};
use mime_guess::from_path;
use std::sync::Arc;

const XML_CD_SCPD: &str = include_str!("../../assets/ContentDirectory.xml");
const XML_CD_SYSTEM_UPDATE: &str = include_str!("../../assets/cd_system_update.xml");
const SOAP_BROWSE_WRAPPER: &str = include_str!("../../assets/soap_browse_wrapper.xml");

pub async fn handle_content_directory() -> impl IntoResponse {
    println!("[INFO] handle_content_directory");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
        XML_CD_SCPD,
    )
}

pub async fn handle_ctl_content_directory(
    State(state): State<Arc<DlnaState>>, //
    body: String,
) -> impl IntoResponse {
    println!("[INFO] handle_ctl_content_directory");

    // end here if user doesn't not click on a directory
    if !body.contains("Browse") {
        return (
            [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
            XML_CD_SYSTEM_UPDATE.to_string(),
        )
            .into_response();
    }

    // user click on a directory, the object_id should be the directory path
    let object_id = ["<ObjectID>", "&lt;ObjectID&gt;"]
        .iter()
        .find_map(|&pattern| {
            let rest = &body[body.find(pattern)? + pattern.len()..];
            let end = rest.find('<').or_else(|| rest.find("&lt;"))?;
            let id = rest[..end].trim();
            (!id.is_empty()).then(|| id.to_string())
        })
        .unwrap_or_else(|| "0".to_string());

    // init the target_dir from object_id
    let target_dir = if object_id == "0" { "" } else { &object_id };

    // list the target_dir then generate the didl_entries
    let mut didl_entries = String::new();
    if let Ok(entries) = state.source.read_dir(&target_dir).await {
        for entry in entries {
            let safe_name = entry.name.replace('&', ".");

            if entry.is_dir {
                // inject directory template
                didl_entries.push_str(&format!(
                    include_str!("../../assets/didl_container.xml"), //
                    entry.path, safe_name
                ));
            } else {
                // urlencode just the dirname and filename path of the path
                let encoded_path = entry
                    .path //
                    .split('/')
                    .map(urlencoding::encode)
                    .collect::<Vec<_>>()
                    .join("/");
                let stream_url = format!("http://{}/stream/{}", state.host, encoded_path);
                // guess mime type
                let mime = from_path(&entry.path).first_or_octet_stream().to_string();
                // derive upnp_class from top-level type
                let upnp_class = match &mime.split('/').next() {
                    Some("video") => "object.item.videoItem.movie",
                    Some("audio") => "object.item.audioItem.musicTrack",
                    Some("image") => "object.item.imageItem.photo",
                    _ => "object.item",
                };
                println!("{}, {}, {}", entry.path, mime, upnp_class);
                // inject video file template
                didl_entries.push_str(&format!(
                    include_str!("../../assets/didl_item.xml"),
                    entry.path, //
                    safe_name,
                    upnp_class,
                    mime,
                    entry.size,
                    stream_url,
                ));
            }
        }
    }

    // wrap didl_entries to didl_content
    let didl_content = format!(include_str!("../../assets/didl_content.xml"), didl_entries);

    // wrap the didl_content with the soap browser wrapper
    let response_xml = SOAP_BROWSE_WRAPPER.replace("{}", &didl_content);

    // reply
    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
        response_xml,
    )
        .into_response()
}
