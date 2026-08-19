//! Adapter Windows fondé sur `GlobalSystemMediaTransportControls` (GSMTC).
//!
//! Aucune injection, aucun scraping, aucune simulation de raccourci : uniquement l'API
//! publique de sessions média du système.
//!
//! Les objets WinRT sont manipulés dans un thread dédié initialisé en MTA. Le reste du
//! bridge communique avec lui par messages, ce qui évite toute contrainte d'agilité COM
//! sur le runtime asynchrone.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackInfo as PlaybackInfo,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as WinStatus,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::adapters::app_volume::AppVolume;
use crate::adapters::PlaybackAdapter;
use crate::contract::{
    Artwork, BridgeError, BridgeErrorCode, Command, PlaybackCapabilities, PlaybackSnapshot,
    PlaybackSource, PlaybackStatus,
};
use crate::store::PlaybackStore;

/// Identité applicative de Deezer Desktop, relevée pendant le spike M0.
const DEEZER_APP_ID: &str = "com.deezer.deezer-desktop";

/// Cadence d'interrogation de la session. GSMTC est un appel COM local très bon marché ;
/// 400 ms tient largement l'exigence de propagation sous la seconde (§2.3).
const POLL_INTERVAL: Duration = Duration::from_millis(400);

/// Attente entre deux tentatives d'obtention du gestionnaire de sessions média.
const MANAGER_RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// Nombre de ticks sans session Deezer avant de réacquérir le gestionnaire (≈ 5 s).
const MANAGER_REFRESH_TICKS: u32 = 12;

/// Au-delà, la position rapportée est considérée comme périmée et n'est plus extrapolée.
const MAX_EXTRAPOLATION_MS: i64 = 5_000;

/// Garde-fou mémoire sur la pochette renvoyée par le lecteur.
const MAX_ARTWORK_BYTES: u32 = 4 * 1024 * 1024;

/// Décalage entre l'époque WinRT (1601-01-01) et l'époque Unix, en millisecondes.
const WINRT_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

enum WorkerMessage {
    Refresh,
    Execute {
        command: Command,
        reply: tokio::sync::oneshot::Sender<Result<(), BridgeError>>,
    },
    Shutdown,
}

pub struct WindowsMediaSessionAdapter {
    sender: Mutex<Option<Sender<WorkerMessage>>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl WindowsMediaSessionAdapter {
    pub fn new() -> Self {
        Self {
            sender: Mutex::new(None),
            worker: Mutex::new(None),
        }
    }

    fn send(&self, message: WorkerMessage) -> Result<(), BridgeError> {
        let guard = self.sender.lock().unwrap_or_else(|e| e.into_inner());
        match guard.as_ref() {
            Some(sender) => sender.send(message).map_err(|_| {
                BridgeError::new(
                    BridgeErrorCode::InternalError,
                    "Le service de session média s'est arrêté.",
                    true,
                )
            }),
            None => Err(BridgeError::new(
                BridgeErrorCode::InternalError,
                "Adapter non démarré.",
                true,
            )),
        }
    }
}

impl Default for WindowsMediaSessionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlaybackAdapter for WindowsMediaSessionAdapter {
    fn id(&self) -> &'static str {
        "windows-media-session"
    }

    async fn start(self: Arc<Self>, store: Arc<PlaybackStore>) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel::<WorkerMessage>();
        *self.sender.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

        let handle = std::thread::Builder::new()
            .name("dezzer-media-session".into())
            .spawn(move || worker_main(rx, store))?;

        *self.worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    async fn shutdown(&self) {
        let _ = self.send(WorkerMessage::Shutdown);
        let handle = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(handle) = handle {
            let _ = tokio::task::spawn_blocking(move || handle.join()).await;
        }
    }

    async fn execute(&self, command: Command) -> Result<(), BridgeError> {
        let (reply, wait) = tokio::sync::oneshot::channel();
        self.send(WorkerMessage::Execute { command, reply })?;

        match tokio::time::timeout(Duration::from_secs(5), wait).await {
            Ok(Ok(result)) => {
                let _ = self.send(WorkerMessage::Refresh);
                result
            }
            Ok(Err(_)) => Err(BridgeError::new(
                BridgeErrorCode::InternalError,
                "Réponse perdue par le service de session média.",
                true,
            )),
            Err(_) => Err(BridgeError::new(
                BridgeErrorCode::CommandFailed,
                "Le lecteur n'a pas répondu à temps.",
                true,
            )),
        }
    }
}

// --- Thread WinRT -----------------------------------------------------------------

/// Ce que l'adapter a appris à l'usage, pour corriger des capacités mal déclarées.
#[derive(Debug, Default, Clone, Copy)]
struct LearnedCapabilities {
    succeeded: PlaybackCapabilities,
    failed: PlaybackCapabilities,
}

struct Worker {
    manager: SessionManager,
    store: Arc<PlaybackStore>,
    learned: LearnedCapabilities,
    /// Pochette de la piste courante, conservée pour ne pas relire le flux à chaque tick.
    artwork_cache: Option<(String, Artwork)>,
    last_source: Option<String>,
    /// Ticks consécutifs sans session Deezer, qui déclenchent une réacquisition.
    misses: u32,
}

fn worker_main(rx: mpsc::Receiver<WorkerMessage>, store: Arc<PlaybackStore>) {
    // MTA : les objets GSMTC sont alors utilisables sans pompe de messages.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    let Some(manager) = acquire_manager(&rx, &store) else {
        tracing::info!("thread de session media arrete avant initialisation");
        return;
    };

    let mut worker = Worker {
        manager,
        store,
        learned: LearnedCapabilities::default(),
        artwork_cache: None,
        last_source: None,
        misses: 0,
    };

    worker.publish();

    loop {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(WorkerMessage::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(WorkerMessage::Refresh) | Err(RecvTimeoutError::Timeout) => worker.publish(),
            Ok(WorkerMessage::Execute { command, reply }) => {
                let result = worker.execute(command);
                let _ = reply.send(result);
                worker.publish();
            }
        }
    }

    tracing::info!("thread de session media arrete");
}

/// Obtient le gestionnaire de sessions média, en réessayant indéfiniment.
///
/// Abandonner à la première erreur laissait le bridge aveugle pour toute sa durée de vie,
/// alors que le service média Windows peut n'être prêt que quelques secondes après
/// l'ouverture de session.
fn acquire_manager(
    rx: &mpsc::Receiver<WorkerMessage>,
    store: &Arc<PlaybackStore>,
) -> Option<SessionManager> {
    let mut reported = false;

    loop {
        match SessionManager::RequestAsync().and_then(block_on) {
            Ok(manager) => {
                if reported {
                    tracing::info!("gestionnaire de sessions media enfin disponible");
                }
                return Some(manager);
            }
            Err(error) if !reported => {
                tracing::error!(
                    code = %error.code().0,
                    "gestionnaire de sessions media indisponible, nouvelles tentatives en cours"
                );
                store.apply(PlaybackSnapshot::default());
                store.publish_error(BridgeError::new(
                    BridgeErrorCode::InternalError,
                    "Les sessions média Windows sont inaccessibles.",
                    true,
                ));
                reported = true;
            }
            Err(_) => {}
        }

        match rx.recv_timeout(MANAGER_RETRY_INTERVAL) {
            Ok(WorkerMessage::Shutdown) | Err(RecvTimeoutError::Disconnected) => return None,
            Ok(WorkerMessage::Execute { reply, .. }) => {
                let _ = reply.send(Err(BridgeError::new(
                    BridgeErrorCode::InternalError,
                    "Les sessions média Windows sont inaccessibles.",
                    true,
                )));
            }
            Ok(WorkerMessage::Refresh) | Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

impl Worker {
    fn publish(&mut self) {
        let snapshot = self.snapshot();
        self.store.apply(snapshot);
    }

    fn snapshot(&mut self) -> PlaybackSnapshot {
        let Some(session) = self.select_session() else {
            if self.last_source.take().is_some() {
                tracing::info!("session Deezer perdue");
                self.artwork_cache = None;
            }
            self.misses = self.misses.saturating_add(1);
            if self.misses >= MANAGER_REFRESH_TICKS {
                self.refresh_manager();
            }
            return PlaybackSnapshot::default();
        };

        self.misses = 0;

        let source_id = session
            .SourceAppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();

        if self.last_source.as_deref() != Some(source_id.as_str()) {
            tracing::info!(source = %source_id, "session Deezer detectee");
            self.last_source = Some(source_id.clone());
        }

        match self.read_session(&session) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                tracing::debug!(code = %error.code().0, "lecture de session en echec");
                PlaybackSnapshot::default()
            }
        }
    }

    /// Réacquiert le gestionnaire de sessions après une absence prolongée de Deezer.
    ///
    /// Un gestionnaire obtenu avant le lancement de Deezer peut continuer d'énumérer une
    /// liste de sessions périmée ; en redemander un force une énumération neuve.
    fn refresh_manager(&mut self) {
        self.misses = 0;
        match SessionManager::RequestAsync().and_then(block_on) {
            Ok(manager) => {
                self.manager = manager;
                tracing::debug!("gestionnaire de sessions media reacquis");
            }
            Err(error) => {
                tracing::debug!(
                    code = %error.code().0,
                    "reacquisition du gestionnaire de sessions en echec"
                );
            }
        }
    }

    /// Politique de sélection (§7.2) : uniquement Deezer, en préférant la session en
    /// lecture, puis la plus récemment mise à jour. Spotify ou un navigateur ne doivent
    /// jamais être sélectionnés.
    fn select_session(&self) -> Option<Session> {
        let sessions = self.manager.GetSessions().ok()?;
        let count = sessions.Size().ok()?;

        let mut best: Option<(u8, i64, Session)> = None;
        for index in 0..count {
            let Ok(session) = sessions.GetAt(index) else {
                continue;
            };
            let Ok(source) = session.SourceAppUserModelId() else {
                continue;
            };
            let source = source.to_string();
            if !is_deezer_source(&source) {
                continue;
            }

            let playing = session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .map(|status| status == WinStatus::Playing)
                .unwrap_or(false);

            let updated_at = session
                .GetTimelineProperties()
                .and_then(|tl| tl.LastUpdatedTime())
                .map(|dt| dt.UniversalTime)
                .unwrap_or(0);

            // Priorité 1 : exactitude de l'identifiant. Priorité 2 : en lecture.
            let rank = u8::from(source == DEEZER_APP_ID) * 2 + u8::from(playing);
            let candidate = (rank, updated_at, session);
            best = match best {
                Some(current) if (current.0, current.1) >= (candidate.0, candidate.1) => {
                    Some(current)
                }
                _ => Some(candidate),
            };
        }

        best.map(|(_, _, session)| session)
    }

    fn read_session(&mut self, session: &Session) -> windows::core::Result<PlaybackSnapshot> {
        let info = session.GetPlaybackInfo()?;
        let status = match info.PlaybackStatus()? {
            WinStatus::Playing => PlaybackStatus::Playing,
            WinStatus::Paused => PlaybackStatus::Paused,
            WinStatus::Stopped | WinStatus::Closed => PlaybackStatus::Stopped,
            _ => PlaybackStatus::Paused,
        };

        let declared = read_capabilities(&info)?;
        let volume = AppVolume::get();
        let capabilities = effective_capabilities(declared, self.learned, volume.is_some());

        let timeline = session.GetTimelineProperties()?;
        let duration_ms =
            positive_ms(timeline.EndTime()?.Duration - timeline.StartTime()?.Duration);
        let raw_position_ms = positive_ms(timeline.Position()?.Duration);
        let position_ms = raw_position_ms.map(|position| {
            extrapolate_position(
                position,
                timeline.LastUpdatedTime().map(|dt| dt.UniversalTime).ok(),
                status,
                duration_ms,
                now_unix_ms(),
            )
        });

        let props = block_on(session.TryGetMediaPropertiesAsync()?)?;
        let title = non_empty(props.Title()?.to_string());
        let artist = non_empty(props.Artist()?.to_string());
        let album = non_empty(props.AlbumTitle()?.to_string());
        let track_id = track_identity(title.as_deref(), artist.as_deref(), album.as_deref());

        let artwork = match &track_id {
            Some(id) => self.artwork_for(id, session),
            None => None,
        };

        Ok(PlaybackSnapshot {
            source: PlaybackSource::DeezerDesktop,
            source_label: Some("Deezer Desktop".into()),
            available: true,
            status,
            track_id,
            title,
            artist,
            album,
            artwork,
            position_ms,
            duration_ms,
            // Le volume ne vient pas de GSMTC mais du mixeur Windows (Core Audio).
            volume,
            capabilities,
        })
    }

    fn artwork_for(&mut self, track_id: &str, session: &Session) -> Option<Artwork> {
        if let Some((cached_id, artwork)) = &self.artwork_cache {
            if cached_id == track_id {
                return Some(artwork.clone());
            }
        }

        match read_thumbnail(session) {
            Ok(Some(artwork)) => {
                self.artwork_cache = Some((track_id.to_string(), artwork.clone()));
                Some(artwork)
            }
            Ok(None) => {
                // Piste sans pochette : on memorise l'absence pour ne pas relire chaque tick.
                self.artwork_cache = None;
                None
            }
            Err(error) => {
                tracing::debug!(code = %error.code().0, "pochette illisible");
                None
            }
        }
    }

    fn execute(&mut self, command: Command) -> Result<(), BridgeError> {
        let Some(session) = self.select_session() else {
            return Err(BridgeError::player_not_found());
        };

        let outcome = match command {
            Command::PlayPause => session.TryTogglePlayPauseAsync().and_then(block_on),
            Command::Next => session.TrySkipNextAsync().and_then(block_on),
            Command::Previous => session.TrySkipPreviousAsync().and_then(block_on),
            Command::Stop => session.TryStopAsync().and_then(block_on),
            Command::Seek { position_ms } => session
                .TryChangePlaybackPositionAsync(ms_to_ticks(position_ms))
                .and_then(block_on),
            Command::SetVolume { value } => {
                return match AppVolume::set(value) {
                    Ok(()) => Ok(()),
                    Err(error) => Err(crate::adapters::command_failed(
                        command,
                        format!("mixeur Windows : {:#x}", error.code().0),
                    )),
                };
            }
        };

        match outcome {
            Ok(true) => {
                self.learned.succeeded = self.learned.succeeded.union(capability_of(command));
                Ok(())
            }
            Ok(false) => {
                self.learned.failed = self.learned.failed.union(capability_of(command));
                Err(BridgeError::unsupported(command.name()))
            }
            Err(error) => Err(crate::adapters::command_failed(
                command,
                format!("HRESULT {:#x}", error.code().0),
            )),
        }
    }
}

// --- Fonctions pures, testables sans Windows ---------------------------------------

/// Attend une opération WinRT depuis le thread dédié, qui n'est pas asynchrone.
fn block_on<T, F>(operation: F) -> windows::core::Result<T>
where
    F: std::future::IntoFuture<Output = windows::core::Result<T>>,
{
    futures_executor::block_on(operation.into_future())
}

/// Traduit les capacités déclarées par la session vers le contrat partagé.
fn read_capabilities(info: &PlaybackInfo) -> windows::core::Result<PlaybackCapabilities> {
    let controls = info.Controls()?;
    Ok(PlaybackCapabilities {
        play_pause: controls.IsPlayEnabled()?
            || controls.IsPauseEnabled()?
            || controls.IsPlayPauseToggleEnabled()?,
        next: controls.IsNextEnabled()?,
        previous: controls.IsPreviousEnabled()?,
        stop: controls.IsStopEnabled()?,
        seek: controls.IsPlaybackPositionEnabled()?,
        shuffle: controls.IsShuffleEnabled()?,
        repeat: controls.IsRepeatEnabled()?,
        // GSMTC n'expose aucun contrôle de volume (matrice de compatibilité M0).
        volume: false,
    })
}

pub fn is_deezer_source(source: &str) -> bool {
    let source = source.trim();
    source.eq_ignore_ascii_case(DEEZER_APP_ID) || source.to_ascii_lowercase().contains("deezer")
}

fn capability_of(command: Command) -> PlaybackCapabilities {
    let mut caps = PlaybackCapabilities::default();
    match command {
        Command::PlayPause => caps.play_pause = true,
        Command::Next => caps.next = true,
        Command::Previous => caps.previous = true,
        Command::Stop => caps.stop = true,
        Command::Seek { .. } => caps.seek = true,
        Command::SetVolume { .. } => caps.volume = true,
    }
    caps
}

/// Corrige les capacités déclarées à partir de ce qui a été observé.
///
/// Deezer déclare en permanence `IsPreviousEnabled = false` alors que la commande
/// fonctionne (spike M0). On postule donc `previous` dès que `next` est disponible, tant
/// qu'un échec réel n'a pas été observé.
fn effective_capabilities(
    declared: PlaybackCapabilities,
    learned: LearnedCapabilities,
    volume_available: bool,
) -> PlaybackCapabilities {
    let assumed = PlaybackCapabilities {
        previous: declared.next,
        ..Default::default()
    };

    let mut caps = declared.union(assumed).union(learned.succeeded);

    let failed = learned.failed;
    caps.play_pause &= !failed.play_pause;
    caps.next &= !failed.next;
    caps.previous &= !failed.previous;
    caps.stop &= !failed.stop;
    caps.seek &= !failed.seek;
    caps.shuffle &= !failed.shuffle;
    caps.repeat &= !failed.repeat;
    // Le volume ne vient pas de GSMTC : il depend de l'existence d'une session Core Audio.
    caps.volume = volume_available && !failed.volume;
    caps
}

fn ms_to_ticks(ms: u64) -> i64 {
    (ms as i64).saturating_mul(10_000)
}

fn positive_ms(ticks: i64) -> Option<u64> {
    if ticks <= 0 {
        return None;
    }
    Some((ticks / 10_000) as u64)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn track_identity(
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
) -> Option<String> {
    // Deezer ne fournit pas d'identifiant de piste : on en derive un depuis les metadonnees.
    if title.is_none() && artist.is_none() {
        return None;
    }
    let material = format!(
        "{}\u{1f}{}\u{1f}{}",
        title.unwrap_or_default(),
        artist.unwrap_or_default(),
        album.unwrap_or_default()
    );
    let digest = Sha256::digest(material.as_bytes());
    Some(digest.iter().take(8).map(|b| format!("{b:02x}")).collect())
}

fn winrt_to_unix_ms(universal_time: i64) -> i64 {
    universal_time / 10_000 - WINRT_EPOCH_OFFSET_MS
}

fn now_unix_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// La position rapportée par GSMTC date de `last_updated`. On l'avance de l'écart écoulé
/// pendant la lecture, en bornant l'extrapolation pour ne jamais inventer une progression.
fn extrapolate_position(
    position_ms: u64,
    last_updated: Option<i64>,
    status: PlaybackStatus,
    duration_ms: Option<u64>,
    now_ms: i64,
) -> u64 {
    let mut position = position_ms;

    if status == PlaybackStatus::Playing {
        if let Some(universal_time) = last_updated {
            let elapsed = now_ms - winrt_to_unix_ms(universal_time);
            if (0..=MAX_EXTRAPOLATION_MS).contains(&elapsed) {
                position = position.saturating_add(elapsed as u64);
            }
        }
    }

    match duration_ms {
        Some(duration) => position.min(duration),
        None => position,
    }
}

fn read_thumbnail(session: &Session) -> windows::core::Result<Option<Artwork>> {
    let props = block_on(session.TryGetMediaPropertiesAsync()?)?;
    let Ok(reference) = props.Thumbnail() else {
        return Ok(None);
    };

    let stream = block_on(reference.OpenReadAsync()?)?;
    let size = stream.Size()?;
    if size == 0 || size > MAX_ARTWORK_BYTES as u64 {
        return Ok(None);
    }

    let input = stream.GetInputStreamAt(0)?;
    let reader = DataReader::CreateDataReader(&input)?;
    let loaded = block_on(reader.LoadAsync(size as u32)?)?;
    if loaded == 0 {
        return Ok(None);
    }

    let mut bytes = vec![0u8; loaded as usize];
    reader.ReadBytes(&mut bytes)?;

    let declared_mime = stream
        .ContentType()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let mime = sniff_mime(&bytes, &declared_mime);

    Ok(Some(Artwork {
        key: artwork_key(&bytes),
        mime,
        bytes,
    }))
}

fn artwork_key(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Le type déclaré par le lecteur peut être vide ; on ne sert alors que des types image
/// reconnus, jamais une valeur arbitraire venue de la session.
fn sniff_mime(bytes: &[u8], declared: &str) -> String {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg".into();
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "image/png".into();
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return "image/webp".into();
    }
    if bytes.starts_with(b"GIF8") {
        return "image/gif".into();
    }

    match declared.trim() {
        "image/jpeg" | "image/png" | "image/webp" | "image/gif" => declared.trim().to_string(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ne_selectionne_que_des_sessions_deezer() {
        assert!(is_deezer_source("com.deezer.deezer-desktop"));
        assert!(is_deezer_source("Deezer.Deezer_abc123!App"));
        assert!(!is_deezer_source("Spotify.exe"));
        assert!(!is_deezer_source("Chrome"));
        assert!(!is_deezer_source(
            "Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"
        ));
    }

    #[test]
    fn postule_precedent_quand_suivant_est_disponible() {
        let declared = PlaybackCapabilities {
            play_pause: true,
            next: true,
            previous: false,
            seek: true,
            ..Default::default()
        };
        let caps = effective_capabilities(declared, LearnedCapabilities::default(), false);
        assert!(
            caps.previous,
            "le piege Deezer sur IsPreviousEnabled doit etre compense"
        );
        assert!(!caps.volume, "sans session Core Audio, pas de volume");
    }

    #[test]
    fn active_le_volume_uniquement_si_une_session_audio_existe() {
        let declared = PlaybackCapabilities::default();
        assert!(!effective_capabilities(declared, LearnedCapabilities::default(), false).volume);
        assert!(effective_capabilities(declared, LearnedCapabilities::default(), true).volume);
    }

    #[test]
    fn desactive_une_capacite_apres_un_echec_reel() {
        let declared = PlaybackCapabilities {
            next: true,
            ..Default::default()
        };
        let learned = LearnedCapabilities {
            failed: PlaybackCapabilities {
                previous: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let caps = effective_capabilities(declared, learned, false);
        assert!(!caps.previous);
        assert!(caps.next);
    }

    #[test]
    fn conserve_une_capacite_observee_meme_si_elle_reste_non_declaree() {
        let learned = LearnedCapabilities {
            succeeded: PlaybackCapabilities {
                seek: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let caps = effective_capabilities(PlaybackCapabilities::default(), learned, false);
        assert!(caps.seek);
    }

    #[test]
    fn convertit_l_epoque_winrt_en_epoque_unix() {
        // 2026-08-19T13:20:00Z en ticks WinRT.
        let unix_ms = 1_787_923_200_000i64;
        let universal = (unix_ms + WINRT_EPOCH_OFFSET_MS) * 10_000;
        assert_eq!(winrt_to_unix_ms(universal), unix_ms);
    }

    #[test]
    fn extrapole_la_position_uniquement_pendant_la_lecture() {
        let now = 1_787_923_200_000i64;
        let updated = (now - 800 + WINRT_EPOCH_OFFSET_MS) * 10_000;

        let playing = extrapolate_position(
            10_000,
            Some(updated),
            PlaybackStatus::Playing,
            Some(200_000),
            now,
        );
        assert_eq!(playing, 10_800);

        let paused = extrapolate_position(
            10_000,
            Some(updated),
            PlaybackStatus::Paused,
            Some(200_000),
            now,
        );
        assert_eq!(paused, 10_000, "en pause la position ne doit pas avancer");
    }

    #[test]
    fn borne_l_extrapolation_et_la_duree() {
        let now = 1_787_923_200_000i64;
        let stale = (now - 60_000 + WINRT_EPOCH_OFFSET_MS) * 10_000;
        assert_eq!(
            extrapolate_position(10_000, Some(stale), PlaybackStatus::Playing, None, now),
            10_000,
            "une position perimee ne doit pas etre extrapolee"
        );

        let fresh = (now - 5_000 + WINRT_EPOCH_OFFSET_MS) * 10_000;
        assert_eq!(
            extrapolate_position(
                199_000,
                Some(fresh),
                PlaybackStatus::Playing,
                Some(200_000),
                now
            ),
            200_000,
            "la position doit etre bornee a la duree"
        );
    }

    #[test]
    fn derive_un_identifiant_de_piste_stable() {
        let a = track_identity(Some("Titre"), Some("Artiste"), Some("Album"));
        let b = track_identity(Some("Titre"), Some("Artiste"), Some("Album"));
        let c = track_identity(Some("Titre"), Some("Autre"), Some("Album"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(track_identity(None, None, Some("Album")).is_none());
    }

    #[test]
    fn ne_sert_jamais_un_type_mime_arbitraire() {
        assert_eq!(sniff_mime(&[0xFF, 0xD8, 0xFF, 0x00], ""), "image/jpeg");
        assert_eq!(sniff_mime(&[0x89, b'P', b'N', b'G'], ""), "image/png");
        assert_eq!(
            sniff_mime(b"<script>", "text/html; charset=utf-8"),
            "application/octet-stream"
        );
    }

    #[test]
    fn ignore_les_durees_nulles_plutot_que_de_renvoyer_zero() {
        assert_eq!(positive_ms(0), None);
        assert_eq!(positive_ms(-1), None);
        assert_eq!(positive_ms(1_230_000), Some(123));
    }
}
