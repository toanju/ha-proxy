mod config;
mod filter;
mod proxy;

use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use reqwest::Client;
use secrecy::SecretString;
use serde_json::json;
use tower_http::trace::TraceLayer;
use tracing::info;

use config::{AllowEntry, Config};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

struct AppState {
    ha_url: String,
    token: SecretString,
    allow: Vec<AllowEntry>,
    client: Client,
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn services_handler(
    State(state): State<Arc<AppState>>,
    Path((domain, service)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 1. Allow-list check
    if let Err(e) = filter::check(&state.allow, &domain, &service) {
        return json_error(StatusCode::FORBIDDEN, e.message());
    }

    // 2. Extract Content-Type to forward verbatim
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    // 3. Forward to Home Assistant
    match proxy::forward(
        &state.client,
        &state.ha_url,
        &state.token,
        &domain,
        &service,
        content_type,
        body,
    )
    .await
    {
        Ok((status, response_body)) => {
            (status, Body::from(response_body)).into_response()
        }
        Err((status, message)) => json_error(status, message),
    }
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

async fn health() -> Response {
    (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
}

// ---------------------------------------------------------------------------
// Fallback — any unmatched route
// ---------------------------------------------------------------------------

async fn fallback(_req: Request) -> Response {
    json_error(StatusCode::NOT_FOUND, "not found")
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ha_proxy=info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::load()?;
    let token = config::load_token(&cfg.token_file)?;

    let state = Arc::new(AppState {
        ha_url: cfg.ha_url.clone(),
        token,
        allow: cfg.allow.clone(),
        client: Client::builder()
            .build()
            .expect("failed to build HTTP client"),
    });

    let app = Router::new()
        .route("/api/services/{domain}/{service}", post(services_handler))
        .route("/health", get(health))
        .fallback(fallback)
        .layer(DefaultBodyLimit::max(cfg.max_body_bytes))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
    info!(listen = %cfg.listen, ha_url = %cfg.ha_url, "ha-proxy starting");

    axum::serve(listener, app).await?;

    Ok(())
}
