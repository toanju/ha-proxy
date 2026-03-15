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
// CLI argument parsing (no extra dependencies)
// ---------------------------------------------------------------------------

/// Parse `-c <path>` / `--config <path>` from the process arguments.
/// Returns the config file path, defaulting to `"config.toml"`.
fn config_path_from_args() -> anyhow::Result<String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--config" => {
                let path = args.next().ok_or_else(|| {
                    anyhow::anyhow!("flag '{}' requires a value (path to config file)", arg)
                })?;
                return Ok(path);
            }
            other if other.starts_with("--config=") => {
                return Ok(other.trim_start_matches("--config=").to_string());
            }
            "-h" | "--help" => {
                eprintln!("Usage: ha-proxy [-c|--config <path>]");
                eprintln!();
                eprintln!("  -c, --config <path>  Path to the TOML config file (default: config.toml)");
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown argument '{}' (use --help for usage)", other);
            }
        }
    }
    Ok("config.toml".to_string())
}

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
        proxy::ForwardRequest { domain: &domain, service: &service, content_type, body },
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

    let config_path = config_path_from_args()?;
    let cfg = Config::load(&config_path)?;
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

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Resolves when SIGTERM or Ctrl-C (SIGINT) is received.
async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("received Ctrl-C, shutting down"); },
        _ = terminate => { info!("received SIGTERM, shutting down"); },
    }
}
