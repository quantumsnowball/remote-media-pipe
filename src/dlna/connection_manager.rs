use axum::{http::header, response::IntoResponse};

const XML_CM_SCPD: &str = include_str!("../../assets/ConnectionManager.xml");
const XML_CM_SOAP_RESP: &str = include_str!("../../assets/cm_soap_response.xml");

pub async fn handle_connection_manager() -> impl IntoResponse {
    println!("[INFO] handle_connection_manager");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
        XML_CM_SCPD,
    )
}

pub async fn handle_ctl_connection_manager() -> impl IntoResponse {
    println!("[INFO] handle_ctl_connection_manager");

    (
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")], //
        XML_CM_SOAP_RESP,
    )
}
