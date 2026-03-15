use axum::http::StatusCode;
use bytes::Bytes;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use tracing::error;

/// Per-request data forwarded to the Home Assistant services API.
pub struct ForwardRequest<'a> {
    /// Service domain, e.g. `light`.
    pub domain: &'a str,
    /// Service name, e.g. `turn_on`.
    pub service: &'a str,
    /// Value of the incoming `Content-Type` header, if present.
    pub content_type: Option<String>,
    /// Raw request body forwarded verbatim.
    pub body: Bytes,
}

/// Forward a POST request to the Home Assistant services API and return the
/// upstream status code together with the raw response body.
///
/// # Arguments
/// * `client`  – shared `reqwest::Client` (connection-pooled)
/// * `ha_url`  – validated base URL of the HA instance, e.g. `http://homeassistant.local:8123`
/// * `token`   – bearer token; the raw value is exposed only when building the header
/// * `req`     – per-request data (domain, service, content-type, body)
pub async fn forward(
    client: &Client,
    ha_url: &str,
    token: &SecretString,
    req: ForwardRequest<'_>,
) -> Result<(StatusCode, Bytes), (StatusCode, &'static str)> {
    let url = format!("{}/api/services/{}/{}", ha_url, req.domain, req.service);

    let mut request = client
        .post(&url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token.expose_secret()),
        );

    if let Some(ct) = req.content_type {
        request = request.header(reqwest::header::CONTENT_TYPE, ct);
    }

    let response = request
        .body(req.body)
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, url = %url, "upstream request failed");
            (StatusCode::BAD_GATEWAY, "upstream error")
        })?;

    let upstream_status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    let response_body = response.bytes().await.map_err(|e| {
        error!(error = %e, "failed to read upstream response body");
        (StatusCode::BAD_GATEWAY, "upstream error")
    })?;

    Ok((upstream_status, response_body))
}
