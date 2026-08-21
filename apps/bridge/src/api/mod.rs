//! API HTTP locale.

pub mod auth;
pub mod ws;

use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tower_http::set_header::SetResponseHeaderLayer;

use crate::adapters::PlaybackAdapter;
use crate::config::Config;
use crate::contract::{
    BridgeError, BridgeErrorCode, Command, HealthResponse, NowPlayingState, CONTRACT_VERSION,
    SCHEMA_VERSION,
};
use crate::store::PlaybackStore;

/// Les corps de requête de contrôle sont minuscules ; tout dépassement est suspect.
const MAX_BODY_BYTES: usize = 4 * 1024;

/// Délai laissé à l'adapter pour propager l'effet d'une commande avant de renvoyer l'état.
/// Le spike M0 a mesuré 33 ms sur Deezer Desktop.
const COMMAND_SETTLE_MS: u64 = 150;

pub struct AppState {
    pub config: Arc<Config>,
    pub store: Arc<PlaybackStore>,
    pub adapter: Arc<dyn PlaybackAdapter>,
    pub started_at: Instant,
    pub ws_clients: AtomicUsize,
    port: AtomicU16,
}

impl AppState {
    pub fn new(
        config: Arc<Config>,
        store: Arc<PlaybackStore>,
        adapter: Arc<dyn PlaybackAdapter>,
    ) -> Self {
        Self {
            config,
            store,
            adapter,
            started_at: Instant::now(),
            ws_clients: AtomicUsize::new(0),
            port: AtomicU16::new(0),
        }
    }

    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::Relaxed);
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::Relaxed)
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/health", get(health))
        .route("/v1/state", get(get_state))
        .route("/v1/capabilities", get(get_capabilities))
        .route("/v1/artwork/{key}", get(get_artwork))
        .route("/v1/events", get(ws::handler))
        .route("/v1/controls/play-pause", post(control_play_pause))
        .route("/v1/controls/next", post(control_next))
        .route("/v1/controls/previous", post(control_previous))
        .route("/v1/controls/stop", post(control_stop))
        .route("/v1/controls/volume", post(control_volume))
        .route("/v1/controls/seek", post(control_seek))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate));

    Router::new()
        .merge(protected)
        .fallback(not_found)
        .with_state(state)
}

async fn authenticate(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let headers = request.headers();
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());

    if !auth::host_allowed(host) || !auth::origin_allowed(origin) {
        tracing::warn!(
            origin = origin.unwrap_or("<absente>"),
            "requete refusee : origine ou host non loopback"
        );
        return error_response(
            StatusCode::FORBIDDEN,
            BridgeError::new(
                BridgeErrorCode::TokenInvalid,
                "Origine non autorisee.",
                false,
            ),
        );
    }

    let provided = auth::extract_token(headers, request.uri().query());
    let authorised = provided
        .as_deref()
        .is_some_and(|token| auth::token_matches(&state.config.token, token));

    if !authorised {
        return error_response(
            StatusCode::UNAUTHORIZED,
            BridgeError::new(
                BridgeErrorCode::TokenInvalid,
                "Jeton absent ou invalide.",
                false,
            ),
        );
    }

    next.run(request).await
}

async fn health(State(state): State<Arc<AppState>>) -> Response {
    Json(HealthResponse {
        ready: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        contract_version: CONTRACT_VERSION.to_string(),
        schema_version: SCHEMA_VERSION,
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        adapter: state.adapter.id().to_string(),
        uptime_ms: state.started_at.elapsed().as_millis() as u64,
    })
    .into_response()
}

async fn get_state(State(state): State<Arc<AppState>>) -> Response {
    ok_with_state(state.store.state())
}

async fn get_capabilities(State(state): State<Arc<AppState>>) -> Response {
    Json(json!({ "ok": true, "capabilities": state.store.capabilities() })).into_response()
}

async fn get_artwork(State(state): State<Arc<AppState>>, Path(key): Path<String>) -> Response {
    if !key.chars().all(|c| c.is_ascii_alphanumeric()) || key.len() > 64 {
        return error_response(
            StatusCode::BAD_REQUEST,
            BridgeError::new(BridgeErrorCode::InternalError, "Cle invalide.", false),
        );
    }

    let Some(artwork) = state.store.artwork(&key) else {
        return error_response(
            StatusCode::NOT_FOUND,
            BridgeError::new(
                BridgeErrorCode::PlayerNotFound,
                "Pochette indisponible.",
                false,
            ),
        );
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&artwork.mime)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    // La cle derive du contenu : le cache immuable est sur.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400, immutable"),
    );
    (headers, artwork.bytes.clone()).into_response()
}

async fn control_play_pause(State(state): State<Arc<AppState>>) -> Response {
    run_command(state, Command::PlayPause).await
}

async fn control_next(State(state): State<Arc<AppState>>) -> Response {
    run_command(state, Command::Next).await
}

async fn control_previous(State(state): State<Arc<AppState>>) -> Response {
    run_command(state, Command::Previous).await
}

async fn control_stop(State(state): State<Arc<AppState>>) -> Response {
    run_command(state, Command::Stop).await
}

#[derive(Debug, Deserialize)]
struct VolumeBody {
    value: i64,
}

async fn control_volume(
    State(state): State<Arc<AppState>>,
    body: Result<Json<VolumeBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return bad_request("Corps attendu : { \"value\": 0..100 }");
    };
    if !(0..=100).contains(&body.value) {
        return bad_request("`value` doit etre compris entre 0 et 100.");
    }
    run_command(
        state,
        Command::SetVolume {
            value: body.value as u8,
        },
    )
    .await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekBody {
    position_ms: i64,
}

async fn control_seek(
    State(state): State<Arc<AppState>>,
    body: Result<Json<SeekBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return bad_request("Corps attendu : { \"positionMs\": <entier >= 0> }");
    };
    if body.position_ms < 0 {
        return bad_request("`positionMs` ne peut pas etre negatif.");
    }

    let mut position_ms = body.position_ms as u64;
    if let Some(duration) = state.store.state().duration_ms {
        position_ms = position_ms.min(duration);
    }
    run_command(state, Command::Seek { position_ms }).await
}

async fn run_command(state: Arc<AppState>, command: Command) -> Response {
    let snapshot = state.store.state();

    if !snapshot.available {
        return error_response(StatusCode::CONFLICT, BridgeError::player_not_found());
    }
    if !snapshot.capabilities.supports(command) {
        return error_response(
            StatusCode::CONFLICT,
            BridgeError::unsupported(command.name()),
        );
    }

    match state.adapter.execute(command).await {
        Ok(()) => {
            tokio::time::sleep(std::time::Duration::from_millis(COMMAND_SETTLE_MS)).await;
            ok_with_state(state.store.state())
        }
        Err(error) => {
            tracing::warn!(command = command.name(), code = ?error.code, "commande en echec");
            state.store.publish_error(error.clone());
            let status = match error.code {
                BridgeErrorCode::UnsupportedCapability => StatusCode::CONFLICT,
                BridgeErrorCode::PlayerNotFound => StatusCode::CONFLICT,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error_response(status, error)
        }
    }
}

async fn not_found() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        BridgeError::new(BridgeErrorCode::InternalError, "Route inconnue.", false),
    )
}

fn ok_with_state(state: NowPlayingState) -> Response {
    Json(json!({ "ok": true, "state": state })).into_response()
}

fn bad_request(message: &str) -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        BridgeError::new(BridgeErrorCode::InternalError, message, false),
    )
}

fn error_response(status: StatusCode, error: BridgeError) -> Response {
    (status, Json(json!({ "ok": false, "error": error }))).into_response()
}
