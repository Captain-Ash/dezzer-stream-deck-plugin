//! Bridge Dezzer : point d'entrée bibliothèque, réutilisé par le binaire et les tests.

pub mod adapters;
pub mod api;
pub mod config;
pub mod contract;
pub mod levels;
pub mod logging;
pub mod runtime;
pub mod spectrum;
pub mod store;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::api::AppState;
use crate::config::Config;
use crate::contract::{iso8601, BridgeEvent, CONTRACT_VERSION};
use crate::runtime::{ProcessLock, RuntimeFile, RuntimeInfo};
use crate::store::PlaybackStore;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Cadence de recalage de la position pendant la lecture (§8.4).
const REPUBLISH_INTERVAL: Duration = Duration::from_secs(1);

/// Délai laissé au parent pour réapparaître avant que le bridge ne s'arrête.
const PARENT_GRACE: Duration = Duration::from_secs(3);

pub struct Bridge {
    pub state: Arc<AppState>,
    pub addr: SocketAddr,
    listener: tokio::net::TcpListener,
    _lock: Option<ProcessLock>,
    _runtime_file: Option<RuntimeFile>,
}

impl Bridge {
    /// Prépare le bridge : verrou, adapter, socket. Le port est déjà attribué au retour.
    pub async fn bind(config: Config) -> Result<Self> {
        let config = Arc::new(config);

        let lock = ProcessLock::acquire(&config.data_dir)?;

        let store = Arc::new(PlaybackStore::new());
        let adapter = adapters::select(config.adapter);
        tracing::info!(
            adapter = adapter.id(),
            version = VERSION,
            platform = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            "demarrage du bridge"
        );

        adapter
            .clone()
            .start(store.clone())
            .await
            .context("initialisation de l'adapter de lecture")?;

        let app_state = Arc::new(AppState::new(config.clone(), store, adapter));

        let listener = bind_loopback(config.port).await?;
        let addr = listener.local_addr()?;
        app_state.set_port(addr.port());

        if config.port != 0 && addr.port() != config.port {
            tracing::warn!(
                souhaite = config.port,
                obtenu = addr.port(),
                "port par defaut indisponible : l'URL de l'overlay a change"
            );
        }

        let runtime_file = RuntimeFile::write(
            &config.data_dir,
            &RuntimeInfo {
                pid: std::process::id(),
                port: addr.port(),
                version: VERSION.to_string(),
                contract_version: CONTRACT_VERSION.to_string(),
                started_at: iso8601(Utc::now()),
            },
        )
        .context("ecriture du fichier de disponibilite")?;

        tracing::info!(port = addr.port(), "API locale prete");

        Ok(Self {
            state: app_state,
            addr,
            listener,
            _lock: Some(lock),
            _runtime_file: Some(runtime_file),
        })
    }

    /// Sert l'API jusqu'à l'arrêt demandé (Ctrl+C, disparition du parent, ou `shutdown`).
    pub async fn serve(self) -> Result<()> {
        let state = self.state.clone();
        let overlay_dir = state.config.overlay_dir.clone();
        let router = api::router(state.clone(), &overlay_dir);

        state.store.publish(BridgeEvent::Ready {
            version: VERSION.to_string(),
        });

        let republisher = tokio::spawn({
            let store = state.store.clone();
            async move {
                let mut ticker = tokio::time::interval(REPUBLISH_INTERVAL);
                ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    ticker.tick().await;
                    if store.subscriber_count() > 0 {
                        store.republish();
                    }
                }
            }
        });

        let reason = {
            let parent_pid = state.config.parent_pid;
            let store = state.store.clone();
            axum::serve(self.listener, router)
                .with_graceful_shutdown(async move {
                    let reason = shutdown_signal(parent_pid).await;
                    store.publish(BridgeEvent::Shutdown {
                        reason: reason.clone(),
                    });
                    // Laisse le temps aux clients de recevoir l'evenement d'arret.
                    tokio::time::sleep(Duration::from_millis(150)).await;
                })
                .await
                .context("boucle de service HTTP")?;
            "arret"
        };

        republisher.abort();
        state.adapter.shutdown().await;
        tracing::info!(reason, "bridge arrete");
        Ok(())
    }
}

/// Écoute exclusivement sur la boucle locale (§2.2), en préférant le port fixe pour que
/// l'URL collée dans OBS reste valable d'un démarrage à l'autre.
async fn bind_loopback(port: u16) -> Result<tokio::net::TcpListener> {
    if port == 0 {
        return tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .context("ecoute sur un port ephemere");
    }

    for offset in 0..config::PORT_FALLBACK_ATTEMPTS {
        let candidate = port.saturating_add(offset);
        match tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, candidate)))
            .await
        {
            Ok(listener) => return Ok(listener),
            Err(error) => tracing::debug!(port = candidate, %error, "port indisponible"),
        }
    }

    tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .context("aucun port fixe disponible, et l'attribution ephemere a echoue")
}

async fn shutdown_signal(parent_pid: Option<u32>) -> String {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
        "signal".to_string()
    };

    match parent_pid {
        Some(pid) => tokio::select! {
            reason = ctrl_c => reason,
            _ = runtime::watch_parent(pid, PARENT_GRACE) => "parent-disparu".to_string(),
        },
        None => ctrl_c.await,
    }
}

pub async fn run(config: Config) -> Result<()> {
    Bridge::bind(config).await?.serve().await
}
