//! Bridge Deezer : point d'entrée bibliothèque, réutilisé par le binaire et les tests.

pub mod adapters;
pub mod api;
pub mod config;
pub mod contract;
pub mod logging;
pub mod runtime;
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
        let router = api::router(state.clone());

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

/// Écoute exclusivement sur la boucle locale (§2.2). Le port est éphémère par défaut ;
/// un numéro explicite n'est utilisé que si l'environnement en impose un.
async fn bind_loopback(port: u16) -> Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
        .await
        .with_context(|| format!("ecoute sur 127.0.0.1:{port}"))
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
