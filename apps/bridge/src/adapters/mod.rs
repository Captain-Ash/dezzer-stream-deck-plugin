//! Abstraction des sources de lecture. Aucun code spécifique à l'OS ne doit sortir d'ici.

use std::sync::Arc;

use async_trait::async_trait;

use crate::contract::{BridgeError, BridgeErrorCode, Command};
use crate::store::PlaybackStore;

pub mod mock;
pub mod unavailable;

#[cfg(windows)]
pub mod app_volume;
#[cfg(windows)]
pub mod audio_capture;
#[cfg(windows)]
pub mod windows_media;

#[async_trait]
pub trait PlaybackAdapter: Send + Sync {
    fn id(&self) -> &'static str;

    /// Démarre la surveillance. L'adapter pousse ses instantanés dans le store et gère
    /// lui-même sa boucle interne.
    async fn start(self: Arc<Self>, store: Arc<PlaybackStore>) -> anyhow::Result<()>;

    async fn shutdown(&self);

    async fn execute(&self, command: Command) -> Result<(), BridgeError>;
}

pub fn command_failed(command: Command, detail: impl std::fmt::Display) -> BridgeError {
    BridgeError::new(
        BridgeErrorCode::CommandFailed,
        format!("Echec de la commande {} : {detail}", command.name()),
        true,
    )
}

/// Sélectionne l'adapter selon la configuration et la plateforme.
pub fn select(kind: crate::config::AdapterKind) -> Arc<dyn PlaybackAdapter> {
    use crate::config::AdapterKind;

    match kind {
        AdapterKind::Mock => Arc::new(mock::MockPlaybackAdapter::from_env()),
        AdapterKind::Windows | AdapterKind::Auto => native_adapter(),
    }
}

#[cfg(windows)]
fn native_adapter() -> Arc<dyn PlaybackAdapter> {
    Arc::new(windows_media::WindowsMediaSessionAdapter::new())
}

#[cfg(not(windows))]
fn native_adapter() -> Arc<dyn PlaybackAdapter> {
    Arc::new(unavailable::UnavailableAdapter::new(
        "Cette plateforme n'est pas encore prise en charge.",
    ))
}
