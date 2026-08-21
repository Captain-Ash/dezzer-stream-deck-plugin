//! État de lecture normalisé, numérotation des séquences et diffusion aux clients.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot_shim::RwLock;
use tokio::sync::broadcast;

use crate::contract::{
    iso8601, Artwork, BridgeError, BridgeEvent, NowPlayingState, PlaybackCapabilities,
    PlaybackSnapshot, PlaybackStatus, SCHEMA_VERSION,
};

/// Tolérance au-delà de laquelle une position est considérée comme un vrai saut
/// plutôt qu'une simple progression de la lecture.
const POSITION_DRIFT_TOLERANCE_MS: i64 = 1_500;

/// Nombre maximal d'artworks conservés en mémoire (piste courante + historique proche).
const ARTWORK_CACHE_SIZE: usize = 8;

const EVENT_CHANNEL_CAPACITY: usize = 64;

mod parking_lot_shim {
    //! Fine couche au-dessus de `std::sync::RwLock` pour ignorer l'empoisonnement :
    //! un verrou empoisonné ne doit jamais empêcher le bridge de continuer à servir.
    pub struct RwLock<T>(std::sync::RwLock<T>);

    impl<T> RwLock<T> {
        pub fn new(value: T) -> Self {
            Self(std::sync::RwLock::new(value))
        }

        pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
            self.0.read().unwrap_or_else(|e| e.into_inner())
        }

        pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
            self.0.write().unwrap_or_else(|e| e.into_inner())
        }
    }
}

struct StoreInner {
    state: NowPlayingState,
    /// Dernier instantané brut reçu, utilisé pour détecter les changements significatifs.
    last_snapshot: Option<PlaybackSnapshot>,
    artworks: Vec<Arc<Artwork>>,
    artwork_index: HashMap<String, usize>,
}

pub struct PlaybackStore {
    inner: RwLock<StoreInner>,
    events: broadcast::Sender<BridgeEvent>,
}

impl PlaybackStore {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            inner: RwLock::new(StoreInner {
                state: NowPlayingState::unavailable(0),
                last_snapshot: None,
                artworks: Vec::new(),
                artwork_index: HashMap::new(),
            }),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BridgeEvent> {
        self.events.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.events.receiver_count()
    }

    pub fn state(&self) -> NowPlayingState {
        self.inner.read().state.clone()
    }

    pub fn capabilities(&self) -> PlaybackCapabilities {
        self.inner.read().state.capabilities
    }

    pub fn artwork(&self, key: &str) -> Option<Arc<Artwork>> {
        let inner = self.inner.read();
        inner
            .artwork_index
            .get(key)
            .and_then(|idx| inner.artworks.get(*idx))
            .cloned()
    }

    /// Applique un instantané d'adapter.
    ///
    /// L'état interrogeable est toujours rafraîchi — sinon `GET /v1/state` renverrait une
    /// position figée — mais un événement n'est diffusé que sur changement significatif,
    /// afin de ne pas inonder le plugin pendant la lecture.
    ///
    /// Retourne `true` si un événement a été diffusé.
    pub fn apply(&self, snapshot: PlaybackSnapshot) -> bool {
        let event = {
            let mut inner = self.inner.write();

            let significant = match &inner.last_snapshot {
                None => true,
                Some(previous) => is_significant_change(previous, &snapshot),
            };

            if let Some(artwork) = snapshot.artwork.clone() {
                inner.cache_artwork(artwork);
            }

            let sequence = if significant {
                inner.state.sequence.saturating_add(1)
            } else {
                inner.state.sequence
            };

            inner.state = to_state(&snapshot, sequence);
            inner.last_snapshot = Some(snapshot);

            if !significant {
                return false;
            }
            BridgeEvent::State(inner.state.clone())
        };

        let _ = self.events.send(event);
        true
    }

    /// Rediffuse l'état courant sans incrémenter la séquence : sert au recalage périodique
    /// de la position pendant la lecture (§8.4).
    pub fn republish(&self) {
        let state = {
            let inner = self.inner.read();
            if inner.state.status != PlaybackStatus::Playing {
                return;
            }
            inner.state.clone()
        };
        let _ = self.events.send(BridgeEvent::State(state));
    }

    pub fn publish_error(&self, error: BridgeError) {
        let _ = self.events.send(BridgeEvent::Error(error));
    }

    pub fn publish(&self, event: BridgeEvent) {
        let _ = self.events.send(event);
    }
}

impl Default for PlaybackStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StoreInner {
    fn cache_artwork(&mut self, artwork: Artwork) {
        if self.artwork_index.contains_key(&artwork.key) {
            return;
        }
        if self.artworks.len() >= ARTWORK_CACHE_SIZE {
            let evicted = self.artworks.remove(0);
            self.artwork_index.remove(&evicted.key);
            for idx in self.artwork_index.values_mut() {
                *idx = idx.saturating_sub(1);
            }
        }
        self.artwork_index
            .insert(artwork.key.clone(), self.artworks.len());
        self.artworks.push(Arc::new(artwork));
    }
}

fn to_state(snapshot: &PlaybackSnapshot, sequence: u64) -> NowPlayingState {
    NowPlayingState {
        schema_version: SCHEMA_VERSION,
        source: snapshot.source,
        source_label: snapshot.source_label.clone(),
        available: snapshot.available,
        status: snapshot.status,
        track_id: snapshot.track_id.clone(),
        title: snapshot.title.clone(),
        artist: snapshot.artist.clone(),
        album: snapshot.album.clone(),
        artwork_url: snapshot
            .artwork
            .as_ref()
            .map(|a| format!("/v1/artwork/{}", a.key)),
        position_ms: snapshot.position_ms,
        duration_ms: snapshot.duration_ms,
        volume: snapshot.volume,
        capabilities: snapshot.capabilities,
        updated_at: iso8601(Utc::now()),
        sequence,
    }
}

/// Un simple avancement de la tête de lecture n'est pas un changement significatif.
pub fn is_significant_change(previous: &PlaybackSnapshot, next: &PlaybackSnapshot) -> bool {
    if previous.available != next.available
        || previous.status != next.status
        || previous.source != next.source
        || previous.track_id != next.track_id
        || previous.title != next.title
        || previous.artist != next.artist
        || previous.album != next.album
        || previous.duration_ms != next.duration_ms
        || previous.volume != next.volume
        || previous.capabilities != next.capabilities
    {
        return true;
    }

    let previous_key = previous.artwork.as_ref().map(|a| a.key.as_str());
    let next_key = next.artwork.as_ref().map(|a| a.key.as_str());
    if previous_key != next_key {
        return true;
    }

    match (previous.position_ms, next.position_ms) {
        (None, None) => false,
        (Some(a), Some(b)) => (b as i64 - a as i64).abs() > POSITION_DRIFT_TOLERANCE_MS,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::PlaybackSource;

    fn snapshot(title: &str, position_ms: u64) -> PlaybackSnapshot {
        PlaybackSnapshot {
            source: PlaybackSource::DeezerDesktop,
            available: true,
            status: PlaybackStatus::Playing,
            title: Some(title.to_string()),
            artist: Some("Artiste".into()),
            position_ms: Some(position_ms),
            duration_ms: Some(200_000),
            ..Default::default()
        }
    }

    #[test]
    fn incremente_la_sequence_uniquement_sur_changement_significatif() {
        let store = PlaybackStore::new();

        assert!(store.apply(snapshot("A", 0)));
        assert_eq!(store.state().sequence, 1);

        // Progression normale : pas de nouvel evenement, mais l'etat interrogeable suit.
        assert!(!store.apply(snapshot("A", 900)));
        assert_eq!(store.state().sequence, 1);
        assert_eq!(store.state().position_ms, Some(900));

        // Saut de position : evenement.
        assert!(store.apply(snapshot("A", 60_000)));
        assert_eq!(store.state().sequence, 2);

        // Changement de piste : evenement.
        assert!(store.apply(snapshot("B", 0)));
        assert_eq!(store.state().sequence, 3);
        assert_eq!(store.state().title.as_deref(), Some("B"));
    }

    #[test]
    fn expose_l_artwork_via_une_url_locale_et_jamais_une_url_reseau() {
        let store = PlaybackStore::new();
        let mut snap = snapshot("A", 0);
        snap.artwork = Some(Artwork {
            key: "abc123".into(),
            mime: "image/jpeg".into(),
            bytes: vec![1, 2, 3],
        });
        store.apply(snap);

        let state = store.state();
        assert_eq!(state.artwork_url.as_deref(), Some("/v1/artwork/abc123"));
        assert!(!state.artwork_url.unwrap().starts_with("http"));
        assert_eq!(store.artwork("abc123").unwrap().bytes, vec![1, 2, 3]);
    }

    #[test]
    fn purge_le_cache_d_artwork_sans_perdre_l_entree_courante() {
        let store = PlaybackStore::new();
        for i in 0..(ARTWORK_CACHE_SIZE + 3) {
            let mut snap = snapshot(&format!("piste-{i}"), 0);
            snap.artwork = Some(Artwork {
                key: format!("key-{i}"),
                mime: "image/jpeg".into(),
                bytes: vec![i as u8],
            });
            store.apply(snap);
        }
        assert!(
            store.artwork("key-0").is_none(),
            "la plus ancienne est purgee"
        );
        let last = ARTWORK_CACHE_SIZE + 2;
        assert!(store.artwork(&format!("key-{last}")).is_some());
    }

    #[test]
    fn diffuse_l_etat_aux_abonnes() {
        let store = PlaybackStore::new();
        let mut rx = store.subscribe();
        store.apply(snapshot("A", 0));

        match rx.try_recv().expect("un evenement doit etre diffuse") {
            BridgeEvent::State(state) => assert_eq!(state.title.as_deref(), Some("A")),
            other => panic!("evenement inattendu : {other:?}"),
        }
    }

    #[test]
    fn le_recalage_periodique_ne_change_pas_la_sequence() {
        let store = PlaybackStore::new();
        store.apply(snapshot("A", 0));
        let before = store.state().sequence;
        store.republish();
        assert_eq!(store.state().sequence, before);
    }
}
