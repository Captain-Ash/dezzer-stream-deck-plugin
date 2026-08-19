//! Adapter de repli : aucune source de lecture disponible sur cette plateforme.

use std::sync::Arc;

use async_trait::async_trait;

use crate::adapters::PlaybackAdapter;
use crate::contract::{BridgeError, BridgeErrorCode, Command, PlaybackSnapshot};
use crate::store::PlaybackStore;

pub struct UnavailableAdapter {
    reason: &'static str,
}

impl UnavailableAdapter {
    pub fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

#[async_trait]
impl PlaybackAdapter for UnavailableAdapter {
    fn id(&self) -> &'static str {
        "unavailable"
    }

    async fn start(self: Arc<Self>, store: Arc<PlaybackStore>) -> anyhow::Result<()> {
        store.apply(PlaybackSnapshot::default());
        tracing::warn!(reason = self.reason, "aucun adapter de lecture disponible");
        Ok(())
    }

    async fn shutdown(&self) {}

    async fn execute(&self, _command: Command) -> Result<(), BridgeError> {
        Err(BridgeError::new(
            BridgeErrorCode::PlayerNotFound,
            self.reason,
            false,
        ))
    }
}
