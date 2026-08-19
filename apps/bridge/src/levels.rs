//! Diffusion du spectre audio de Deezer, qui alimente le visualiseur de l'overlay.
//!
//! Le flux est servi sur un WebSocket distinct de `/v1/events` afin que le plugin Stream
//! Deck, qui n'en a aucun usage, ne le reçoive jamais. Rien n'est capturé ni analysé tant
//! qu'aucun overlay n'écoute.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

/// Cadence d'analyse. Trente images par seconde suffisent à l'œil et laissent la machine
/// tranquille.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// Cadence de veille quand aucun overlay n'affiche le spectre.
const IDLE_INTERVAL: Duration = Duration::from_millis(500);

/// Attente avant de retenter l'ouverture de la capture, Deezer pouvant être fermé.
const RETRY_INTERVAL: Duration = Duration::from_secs(2);

const CHANNEL_CAPACITY: usize = 8;

/// Une trame de spectre : un niveau de 0 à 1 par bande, du grave à l'aigu.
pub type Spectrum = Arc<[f32]>;

/// Source partagée des trames de spectre.
pub struct LevelFeed {
    events: broadcast::Sender<Spectrum>,
    listeners: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
}

impl LevelFeed {
    pub fn start() -> Self {
        let (events, _) = broadcast::channel(CHANNEL_CAPACITY);
        let listeners = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));

        spawn_sampler(events.clone(), listeners.clone(), stop.clone());

        Self {
            events,
            listeners,
            stop,
        }
    }

    /// S'abonne au flux. Tant qu'aucun abonnement n'existe, rien n'est capturé.
    pub fn subscribe(&self) -> LevelSubscription {
        self.listeners.fetch_add(1, Ordering::Relaxed);
        LevelSubscription {
            receiver: self.events.subscribe(),
            listeners: self.listeners.clone(),
        }
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Default for LevelFeed {
    fn default() -> Self {
        Self::start()
    }
}

pub struct LevelSubscription {
    pub receiver: broadcast::Receiver<Spectrum>,
    listeners: Arc<AtomicUsize>,
}

impl Drop for LevelSubscription {
    fn drop(&mut self) {
        self.listeners.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(windows)]
fn spawn_sampler(
    events: broadcast::Sender<Spectrum>,
    listeners: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
) {
    use std::time::Instant;

    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    use crate::adapters::audio_capture::{ProcessCapture, SAMPLE_RATE};
    use crate::spectrum::{Analyzer, FFT_SIZE};

    let spawned = std::thread::Builder::new()
        .name("dezzer-audio-spectrum".into())
        .spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }

            let mut analyzer = Analyzer::new(SAMPLE_RATE);
            let mut capture: Option<ProcessCapture> = None;
            let mut samples: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);
            let mut next_attempt = Instant::now();

            while !stop.load(Ordering::Relaxed) {
                if listeners.load(Ordering::Relaxed) == 0 {
                    capture = None;
                    samples.clear();
                    std::thread::sleep(IDLE_INTERVAL);
                    continue;
                }

                if capture.is_none() && Instant::now() >= next_attempt {
                    next_attempt = Instant::now() + RETRY_INTERVAL;
                    capture = open_capture();
                }

                let mut lost = false;
                if let Some(stream) = capture.as_ref() {
                    if stream.drain(&mut |sample| samples.push(sample)).is_err() {
                        lost = true;
                    }
                    if samples.len() > FFT_SIZE {
                        samples.drain(..samples.len() - FFT_SIZE);
                    }
                }

                if lost {
                    tracing::debug!("capture audio perdue, nouvelle tentative");
                    capture = None;
                    samples.clear();
                }

                let bands: Spectrum = if capture.is_some() && samples.len() >= FFT_SIZE {
                    analyzer.analyse(&samples).as_slice().into()
                } else {
                    analyzer.decay().as_slice().into()
                };

                let _ = events.send(bands);
                std::thread::sleep(FRAME_INTERVAL);
            }

            tracing::debug!("thread d'analyse spectrale arrete");
        });

    if let Err(error) = spawned {
        tracing::warn!(%error, "analyse spectrale indisponible");
    }
}

#[cfg(windows)]
fn open_capture() -> Option<crate::adapters::audio_capture::ProcessCapture> {
    use crate::adapters::app_volume;
    use crate::adapters::audio_capture::ProcessCapture;

    let pid = app_volume::deezer_process_id()?;
    match ProcessCapture::open(pid) {
        Ok(stream) => {
            tracing::info!(pid, "capture audio de Deezer ouverte");
            Some(stream)
        }
        Err(error) => {
            tracing::debug!(code = %error.code().0, "capture audio de Deezer indisponible");
            None
        }
    }
}

#[cfg(not(windows))]
fn spawn_sampler(
    _events: broadcast::Sender<Spectrum>,
    _listeners: Arc<AtomicUsize>,
    _stop: Arc<AtomicBool>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_abonnement_libere_son_compteur_a_la_destruction() {
        let feed = LevelFeed::start();
        assert_eq!(feed.listeners.load(Ordering::Relaxed), 0);

        let first = feed.subscribe();
        let second = feed.subscribe();
        assert_eq!(feed.listeners.load(Ordering::Relaxed), 2);

        drop(first);
        assert_eq!(feed.listeners.load(Ordering::Relaxed), 1);

        drop(second);
        assert_eq!(feed.listeners.load(Ordering::Relaxed), 0);

        feed.shutdown();
    }
}
