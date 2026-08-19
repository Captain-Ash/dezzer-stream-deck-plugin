//! Adapter simulé : permet de développer plugin et overlay sans Deezer ni Windows.
//!
//! Activation : `DEZZER_BRIDGE_ADAPTER=mock`.
//!
//! Réglages optionnels :
//! - `DEZZER_MOCK_CAPS` : liste de capacités séparées par des virgules
//!   (`playPause,next,previous,stop,seek,volume`). Défaut : celles réellement observées
//!   sur Deezer Desktop pendant le spike M0.
//! - `DEZZER_MOCK_FLAKY=1` : simule la disparition puis le retour du lecteur.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::adapters::PlaybackAdapter;
use crate::contract::{
    Artwork, BridgeError, Command, PlaybackCapabilities, PlaybackSnapshot, PlaybackSource,
    PlaybackStatus,
};
use crate::store::PlaybackStore;

const TICK: Duration = Duration::from_millis(250);
/// Durée d'un cycle disparition/retour du lecteur en mode instable.
const FLAKY_CYCLE: Duration = Duration::from_secs(20);

struct Track {
    id: &'static str,
    title: &'static str,
    artist: &'static str,
    album: &'static str,
    duration_ms: u64,
    accent: &'static str,
}

const TRACKS: &[Track] = &[
    Track {
        id: "mock-1",
        title: "Nuit Blanche",
        artist: "Astral Kiosk",
        album: "Signaux Faibles",
        duration_ms: 214_000,
        accent: "#7c5cff",
    },
    Track {
        id: "mock-2",
        title: "Un titre volontairement très long pour tester la troncature",
        artist: "Collectif Débordement & Les Invités Surprises",
        album: "",
        duration_ms: 187_500,
        accent: "#ff5c8a",
    },
    Track {
        id: "mock-3",
        title: "<script>alert('xss')</script>",
        artist: "Injection & Co",
        album: "Tests \"hostiles\"",
        duration_ms: 95_000,
        accent: "#2ecc71",
    },
    Track {
        id: "mock-4",
        title: "Sans pochette",
        artist: "Studio Vide",
        album: "Démos",
        duration_ms: 143_000,
        accent: "",
    },
];

struct MockState {
    index: usize,
    position_ms: u64,
    status: PlaybackStatus,
    volume: u8,
    last_tick: Instant,
    started_at: Instant,
    running: bool,
}

pub struct MockPlaybackAdapter {
    state: Mutex<MockState>,
    capabilities: PlaybackCapabilities,
    flaky: bool,
}

impl MockPlaybackAdapter {
    pub fn from_env() -> Self {
        let capabilities = match std::env::var("DEZZER_MOCK_CAPS") {
            Ok(raw) if !raw.trim().is_empty() => parse_capabilities(&raw),
            // Par défaut : exactement ce que le spike M0 a validé sur Deezer Desktop.
            _ => PlaybackCapabilities {
                play_pause: true,
                next: true,
                previous: true,
                stop: true,
                seek: true,
                volume: false,
                shuffle: false,
                repeat: false,
            },
        };

        let flaky = matches!(
            std::env::var("DEZZER_MOCK_FLAKY").as_deref(),
            Ok("1") | Ok("true")
        );

        Self::with_capabilities(capabilities, flaky)
    }

    pub fn with_capabilities(capabilities: PlaybackCapabilities, flaky: bool) -> Self {
        let now = Instant::now();
        Self {
            state: Mutex::new(MockState {
                index: 0,
                position_ms: 0,
                status: PlaybackStatus::Playing,
                volume: 60,
                last_tick: now,
                started_at: now,
                running: true,
            }),
            capabilities,
            flaky,
        }
    }

    fn snapshot(&self) -> PlaybackSnapshot {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.snapshot_locked(&state)
    }

    fn snapshot_locked(&self, state: &MockState) -> PlaybackSnapshot {
        let available = !self.flaky
            || (state.started_at.elapsed().as_secs() % (FLAKY_CYCLE.as_secs() * 2))
                < FLAKY_CYCLE.as_secs();

        if !available {
            return PlaybackSnapshot::default();
        }

        let track = &TRACKS[state.index % TRACKS.len()];
        PlaybackSnapshot {
            source: PlaybackSource::DeezerDesktop,
            source_label: Some("Deezer (simulé)".into()),
            available: true,
            status: state.status,
            track_id: Some(track.id.to_string()),
            title: Some(track.title.to_string()),
            artist: Some(track.artist.to_string()),
            album: (!track.album.is_empty()).then(|| track.album.to_string()),
            artwork: (!track.accent.is_empty()).then(|| placeholder_artwork(track)),
            position_ms: Some(state.position_ms.min(track.duration_ms)),
            duration_ms: Some(track.duration_ms),
            volume: self.capabilities.volume.then_some(state.volume),
            capabilities: self.capabilities,
        }
    }

    fn tick(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let elapsed = state.last_tick.elapsed();
        state.last_tick = Instant::now();

        if state.status != PlaybackStatus::Playing {
            return;
        }

        let duration = TRACKS[state.index % TRACKS.len()].duration_ms;
        state.position_ms += elapsed.as_millis() as u64;
        if state.position_ms >= duration {
            state.index = (state.index + 1) % TRACKS.len();
            state.position_ms = 0;
        }
    }
}

#[async_trait]
impl PlaybackAdapter for MockPlaybackAdapter {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn start(self: Arc<Self>, store: Arc<PlaybackStore>) -> anyhow::Result<()> {
        store.apply(self.snapshot());
        tracing::info!(flaky = self.flaky, "adapter simulé démarré");
        tokio::spawn(run_loop(self, store));
        Ok(())
    }

    async fn shutdown(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.running = false;
    }

    async fn execute(&self, command: Command) -> Result<(), BridgeError> {
        if !self.capabilities.supports(command) {
            return Err(BridgeError::unsupported(command.name()));
        }

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match command {
            Command::PlayPause => {
                state.status = match state.status {
                    PlaybackStatus::Playing => PlaybackStatus::Paused,
                    _ => PlaybackStatus::Playing,
                };
            }
            Command::Next => {
                state.index = (state.index + 1) % TRACKS.len();
                state.position_ms = 0;
                state.status = PlaybackStatus::Playing;
            }
            Command::Previous => {
                // Comportement usuel d'un lecteur : on revient au début avant de reculer.
                if state.position_ms > 3_000 {
                    state.position_ms = 0;
                } else {
                    state.index = (state.index + TRACKS.len() - 1) % TRACKS.len();
                    state.position_ms = 0;
                }
                state.status = PlaybackStatus::Playing;
            }
            Command::Stop => {
                state.status = PlaybackStatus::Stopped;
                state.position_ms = 0;
            }
            Command::Seek { position_ms } => {
                let duration = TRACKS[state.index % TRACKS.len()].duration_ms;
                state.position_ms = position_ms.min(duration);
            }
            Command::SetVolume { value } => {
                state.volume = value.min(100);
            }
        }
        Ok(())
    }
}

/// Boucle de progression du mock. Séparée de `start` pour rester testable.
pub async fn run_loop(adapter: Arc<MockPlaybackAdapter>, store: Arc<PlaybackStore>) {
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        {
            let running = adapter
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .running;
            if !running {
                break;
            }
        }
        adapter.tick();
        store.apply(adapter.snapshot());
    }
}

fn parse_capabilities(raw: &str) -> PlaybackCapabilities {
    let mut caps = PlaybackCapabilities::default();
    for token in raw.split(',') {
        match token.trim().to_ascii_lowercase().as_str() {
            "playpause" | "play-pause" => caps.play_pause = true,
            "next" => caps.next = true,
            "previous" | "prev" => caps.previous = true,
            "stop" => caps.stop = true,
            "seek" => caps.seek = true,
            "volume" => caps.volume = true,
            "shuffle" => caps.shuffle = true,
            "repeat" => caps.repeat = true,
            "" => {}
            other => tracing::warn!(capability = other, "capacite mock inconnue, ignoree"),
        }
    }
    caps
}

fn placeholder_artwork(track: &Track) -> Artwork {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="300" viewBox="0 0 300 300">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
<stop offset="0%" stop-color="{accent}"/><stop offset="100%" stop-color="#101018"/>
</linearGradient></defs>
<rect width="300" height="300" fill="url(#g)"/>
<circle cx="150" cy="150" r="52" fill="none" stroke="#ffffff" stroke-opacity="0.65" stroke-width="10"/>
<circle cx="150" cy="150" r="10" fill="#ffffff" fill-opacity="0.65"/>
</svg>"##,
        accent = track.accent
    );
    let bytes = svg.into_bytes();
    Artwork {
        key: artwork_key(&bytes),
        mime: "image/svg+xml".into(),
        bytes,
    }
}

pub fn artwork_key(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::BridgeErrorCode;

    fn adapter() -> MockPlaybackAdapter {
        MockPlaybackAdapter::with_capabilities(
            PlaybackCapabilities {
                play_pause: true,
                next: true,
                previous: true,
                stop: true,
                seek: true,
                ..Default::default()
            },
            false,
        )
    }

    #[tokio::test]
    async fn bascule_lecture_et_pause() {
        let adapter = adapter();
        assert_eq!(adapter.snapshot().status, PlaybackStatus::Playing);
        adapter.execute(Command::PlayPause).await.unwrap();
        assert_eq!(adapter.snapshot().status, PlaybackStatus::Paused);
        adapter.execute(Command::PlayPause).await.unwrap();
        assert_eq!(adapter.snapshot().status, PlaybackStatus::Playing);
    }

    #[tokio::test]
    async fn refuse_une_commande_hors_capacites() {
        let adapter =
            MockPlaybackAdapter::with_capabilities(PlaybackCapabilities::default(), false);
        let error = adapter.execute(Command::Next).await.unwrap_err();
        assert_eq!(error.code, BridgeErrorCode::UnsupportedCapability);
        assert!(!error.retryable);
    }

    #[tokio::test]
    async fn precedent_revient_au_debut_avant_de_changer_de_piste() {
        let adapter = adapter();
        adapter
            .execute(Command::Seek {
                position_ms: 50_000,
            })
            .await
            .unwrap();
        adapter.execute(Command::Previous).await.unwrap();
        assert_eq!(adapter.snapshot().position_ms, Some(0));
        assert_eq!(adapter.snapshot().track_id.as_deref(), Some("mock-1"));

        adapter.execute(Command::Previous).await.unwrap();
        assert_eq!(adapter.snapshot().track_id.as_deref(), Some("mock-4"));
    }

    #[tokio::test]
    async fn borne_le_seek_a_la_duree_de_la_piste() {
        let adapter = adapter();
        adapter
            .execute(Command::Seek {
                position_ms: 999_999_999,
            })
            .await
            .unwrap();
        let snap = adapter.snapshot();
        assert_eq!(snap.position_ms, snap.duration_ms);
    }

    #[test]
    fn expose_une_piste_sans_pochette_et_une_piste_sans_album() {
        assert!(TRACKS.iter().any(|t| t.accent.is_empty()));
        assert!(TRACKS.iter().any(|t| t.album.is_empty()));
    }

    #[test]
    fn la_cle_d_artwork_depend_du_contenu() {
        assert_eq!(artwork_key(b"a"), artwork_key(b"a"));
        assert_ne!(artwork_key(b"a"), artwork_key(b"b"));
        assert_eq!(artwork_key(b"a").len(), 16);
    }
}
