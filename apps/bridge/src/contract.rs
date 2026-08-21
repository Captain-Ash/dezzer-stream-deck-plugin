//! Contrat de données partagé avec le plugin Stream Deck.
//!
//! Miroir Rust de `packages/playback-contract/src/index.ts`. Toute modification
//! incompatible impose d'incrémenter [`SCHEMA_VERSION`] des deux côtés.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u8 = 1;
pub const CONTRACT_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaybackSource {
    DeezerDesktop,
    DeezerInapp,
    Unknown,
}

/// Capacités *effectives*.
///
/// Le spike M0 a montré que Deezer déclare `IsPreviousEnabled = false` en permanence
/// alors que `TrySkipPreviousAsync` fonctionne. Les adapters doivent donc combiner les
/// capacités déclarées par l'OS avec celles observées à l'usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackCapabilities {
    pub play_pause: bool,
    pub next: bool,
    pub previous: bool,
    pub stop: bool,
    pub volume: bool,
    pub seek: bool,
    pub shuffle: bool,
    pub repeat: bool,
}

impl PlaybackCapabilities {
    pub fn union(self, other: Self) -> Self {
        Self {
            play_pause: self.play_pause || other.play_pause,
            next: self.next || other.next,
            previous: self.previous || other.previous,
            stop: self.stop || other.stop,
            volume: self.volume || other.volume,
            seek: self.seek || other.seek,
            shuffle: self.shuffle || other.shuffle,
            repeat: self.repeat || other.repeat,
        }
    }

    pub fn supports(&self, command: Command) -> bool {
        match command {
            Command::PlayPause => self.play_pause,
            Command::Next => self.next,
            Command::Previous => self.previous,
            Command::Stop => self.stop,
            Command::Seek { .. } => self.seek,
            Command::SetVolume { .. } => self.volume,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    PlayPause,
    Next,
    Previous,
    Stop,
    Seek { position_ms: u64 },
    SetVolume { value: u8 },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::PlayPause => "play-pause",
            Command::Next => "next",
            Command::Previous => "previous",
            Command::Stop => "stop",
            Command::Seek { .. } => "seek",
            Command::SetVolume { .. } => "volume",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingState {
    pub schema_version: u8,
    pub source: PlaybackSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
    pub available: bool,
    pub status: PlaybackStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u8>,

    pub capabilities: PlaybackCapabilities,
    pub updated_at: String,
    pub sequence: u64,
}

impl NowPlayingState {
    pub fn unavailable(sequence: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            source: PlaybackSource::Unknown,
            source_label: None,
            available: false,
            status: PlaybackStatus::Unavailable,
            track_id: None,
            title: None,
            artist: None,
            album: None,
            artwork_url: None,
            position_ms: None,
            duration_ms: None,
            volume: None,
            capabilities: PlaybackCapabilities::default(),
            updated_at: iso8601(Utc::now()),
            sequence,
        }
    }
}

/// Ce que produit un adapter. Le `sequence` et le `updated_at` sont attribués par le store.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackSnapshot {
    pub source: PlaybackSource,
    pub source_label: Option<String>,
    pub available: bool,
    pub status: PlaybackStatus,
    pub track_id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork: Option<Artwork>,
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub volume: Option<u8>,
    pub capabilities: PlaybackCapabilities,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            source: PlaybackSource::Unknown,
            source_label: None,
            available: false,
            status: PlaybackStatus::Unavailable,
            track_id: None,
            title: None,
            artist: None,
            album: None,
            artwork: None,
            position_ms: None,
            duration_ms: None,
            volume: None,
            capabilities: PlaybackCapabilities::default(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Artwork {
    /// Clé stable dérivée du contenu ; sert d'URL locale et de cache-buster.
    pub key: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Artwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artwork")
            .field("key", &self.key)
            .field("mime", &self.mime)
            .field("len", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeErrorCode {
    #[serde(rename = "PLAYER_NOT_FOUND")]
    PlayerNotFound,
    #[serde(rename = "UNSUPPORTED_CAPABILITY")]
    UnsupportedCapability,
    #[serde(rename = "COMMAND_FAILED")]
    CommandFailed,
    #[serde(rename = "TOKEN_INVALID")]
    TokenInvalid,
    #[serde(rename = "BRIDGE_START_FAILED")]
    BridgeStartFailed,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeError {
    pub code: BridgeErrorCode,
    pub message: String,
    pub retryable: bool,
    pub timestamp: String,
}

impl BridgeError {
    pub fn new(code: BridgeErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            timestamp: iso8601(Utc::now()),
        }
    }

    pub fn player_not_found() -> Self {
        Self::new(
            BridgeErrorCode::PlayerNotFound,
            "Aucune session Deezer active.",
            true,
        )
    }

    pub fn unsupported(command: &str) -> Self {
        Self::new(
            BridgeErrorCode::UnsupportedCapability,
            format!("Commande non prise en charge par la session courante : {command}"),
            false,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum BridgeEvent {
    #[serde(rename = "bridge.ready")]
    Ready { version: String },
    #[serde(rename = "playback.state")]
    State(NowPlayingState),
    #[serde(rename = "playback.error")]
    Error(BridgeError),
    #[serde(rename = "bridge.shutdown")]
    Shutdown { reason: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub ready: bool,
    pub version: String,
    pub contract_version: String,
    pub schema_version: u8,
    pub platform: String,
    pub arch: String,
    pub adapter: String,
    pub uptime_ms: u64,
}

pub fn iso8601(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialise_les_enums_selon_le_contrat_typescript() {
        assert_eq!(
            serde_json::to_string(&PlaybackStatus::Playing).unwrap(),
            "\"playing\""
        );
        assert_eq!(
            serde_json::to_string(&PlaybackSource::DeezerDesktop).unwrap(),
            "\"deezer-desktop\""
        );
        assert_eq!(
            serde_json::to_string(&BridgeErrorCode::PlayerNotFound).unwrap(),
            "\"PLAYER_NOT_FOUND\""
        );
    }

    #[test]
    fn omet_les_champs_inconnus_plutot_que_de_les_mettre_a_zero() {
        let state = NowPlayingState::unavailable(0);
        let json = serde_json::to_value(&state).unwrap();
        assert!(json.get("positionMs").is_none());
        assert!(json.get("durationMs").is_none());
        assert!(json.get("title").is_none());
        assert_eq!(json["available"], false);
    }

    #[test]
    fn serialise_les_evenements_avec_type_et_payload() {
        let event = BridgeEvent::Ready {
            version: "0.1.0".into(),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "bridge.ready");
        assert_eq!(json["payload"]["version"], "0.1.0");
    }

    #[test]
    fn union_de_capacites_conserve_les_capacites_observees() {
        let declared = PlaybackCapabilities {
            play_pause: true,
            next: true,
            ..Default::default()
        };
        let observed = PlaybackCapabilities {
            previous: true,
            ..Default::default()
        };
        let effective = declared.union(observed);
        assert!(effective.previous, "la capacite observee doit survivre");
        assert!(effective.play_pause);
        assert!(!effective.volume);
    }
}
