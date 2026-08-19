//! Flux d'événements WebSocket consommé par le plugin Stream Deck et l'overlay.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;

use crate::api::AppState;
use crate::contract::BridgeEvent;

/// Au-delà, on refuse : plugin + quelques Browser Sources OBS suffisent largement.
pub const MAX_CLIENTS: usize = 16;

const HEARTBEAT: Duration = Duration::from_secs(20);

pub async fn handler(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    let clients = state.ws_clients.load(Ordering::Relaxed);
    if clients >= MAX_CLIENTS {
        tracing::warn!(clients, "connexion websocket refusee : trop de clients");
        return (StatusCode::SERVICE_UNAVAILABLE, "too many clients").into_response();
    }

    upgrade
        .max_message_size(16 * 1024)
        .on_upgrade(move |socket| serve(socket, state))
}

async fn serve(socket: WebSocket, state: Arc<AppState>) {
    state.ws_clients.fetch_add(1, Ordering::Relaxed);
    tracing::debug!(
        clients = state.ws_clients.load(Ordering::Relaxed),
        "client websocket connecte"
    );

    // On s'abonne avant d'envoyer l'etat initial pour ne perdre aucun evenement intermediaire.
    let mut events = state.store.subscribe();
    let (mut sink, mut stream) = socket.split();

    let initial = BridgeEvent::State(state.store.state());
    if send(&mut sink, &initial).await.is_err() {
        state.ws_clients.fetch_sub(1, Ordering::Relaxed);
        return;
    }

    let mut heartbeat = tokio::time::interval(HEARTBEAT);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    let shutdown = matches!(event, BridgeEvent::Shutdown { .. });
                    if send(&mut sink, &event).await.is_err() || shutdown {
                        break;
                    }
                }
                // Client trop lent : on le resynchronise avec l'etat courant plutot que
                // de lui envoyer un historique perime.
                Err(RecvError::Lagged(skipped)) => {
                    tracing::debug!(skipped, "client websocket en retard, resynchronisation");
                    let resync = BridgeEvent::State(state.store.state());
                    if send(&mut sink, &resync).await.is_err() {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                // Le flux est unidirectionnel : tout message client est ignore.
                Some(Ok(_)) => {}
            },
            _ = heartbeat.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }

    state.ws_clients.fetch_sub(1, Ordering::Relaxed);
    tracing::debug!(
        clients = state.ws_clients.load(Ordering::Relaxed),
        "client websocket deconnecte"
    );
}

async fn send<S>(sink: &mut S, event: &BridgeEvent) -> Result<(), ()>
where
    S: SinkExt<Message> + Unpin,
{
    let payload = match serde_json::to_string(event) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::error!(%error, "serialisation d'evenement impossible");
            return Ok(());
        }
    };
    sink.send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

/// Flux du niveau audio, séparé de `/v1/events` : seul l'overlay en waveform s'y abonne,
/// et la mesure ne tourne que tant qu'au moins un client écoute.
pub async fn levels(State(state): State<Arc<AppState>>, upgrade: WebSocketUpgrade) -> Response {
    let clients = state.ws_clients.load(Ordering::Relaxed);
    if clients >= MAX_CLIENTS {
        tracing::warn!(clients, "connexion websocket refusee : trop de clients");
        return (StatusCode::SERVICE_UNAVAILABLE, "too many clients").into_response();
    }

    upgrade
        .max_message_size(1024)
        .on_upgrade(move |socket| serve_levels(socket, state))
}

async fn serve_levels(socket: WebSocket, state: Arc<AppState>) {
    state.ws_clients.fetch_add(1, Ordering::Relaxed);

    let mut subscription = state.levels.subscribe();
    let (mut sink, mut stream) = socket.split();

    loop {
        tokio::select! {
            frame = subscription.receiver.recv() => match frame {
                Ok(bands) => {
                    if sink.send(Message::Text(spectrum_frame(&bands).into())).await.is_err() {
                        break;
                    }
                }
                // Un client en retard rattrape sur la trame suivante : l'historique d'un
                // spectre n'a aucun intérêt une fois périmé.
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }

    state.ws_clients.fetch_sub(1, Ordering::Relaxed);
}

/// Trois décimales suffisent pour un visualiseur et divisent la taille de trame par deux.
fn spectrum_frame(bands: &[f32]) -> String {
    let mut payload = String::with_capacity(bands.len() * 6 + 48);
    payload.push_str(r#"{"type":"playback.spectrum","payload":{"bands":["#);
    for (index, band) in bands.iter().enumerate() {
        if index > 0 {
            payload.push(',');
        }
        payload.push_str(&format!("{band:.3}"));
    }
    payload.push_str("]}}");
    payload
}
