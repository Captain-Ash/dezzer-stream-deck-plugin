//! Capture du flux audio de Deezer, pour l'analyse spectrale de l'overlay.
//!
//! Windows sait isoler la boucle audio d'un seul processus depuis Windows 10 2004
//! (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`) : le visualiseur ne réagit donc jamais
//! au reste du système, ni à Discord, ni aux notifications, ni au micro.
//!
//! Aucun périphérique virtuel n'est installé, aucun pilote : c'est l'API publique de
//! capture applicative, celle qu'utilisent les enregistreurs de jeu.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use windows::core::{implement, Interface, Result, HRESULT, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE};
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, WAVE_FORMAT_PCM,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Threading::CreateEventW;

pub const SAMPLE_RATE: u32 = 44_100;

const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;

/// Tampon demandé, en unités de 100 ns. 200 ms laissent une marge confortable au regard
/// des ~33 ms qui séparent deux analyses.
const BUFFER_DURATION_HNS: i64 = 2_000_000;

/// Au-delà, on considère que Windows ne rendra jamais la main.
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(3);

const VT_BLOB: u16 = 65;

/// Disposition binaire d'un `PROPVARIANT` de type `VT_BLOB`.
///
/// `windows-rs` n'expose pas les champs de `PROPVARIANT` ; l'API attendant un pointeur
/// brut, on reconstruit la structure telle que la définit `propidl.h`.
#[repr(C)]
struct BlobPropVariant {
    vt: u16,
    reserved1: u16,
    reserved2: u16,
    reserved3: u16,
    size: u32,
    _padding: u32,
    data: *mut u8,
}

const _: () = assert!(size_of::<BlobPropVariant>() == size_of::<PROPVARIANT>());

#[derive(Default)]
struct Signal {
    completed: Mutex<bool>,
    ready: Condvar,
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct CompletionHandler {
    signal: Arc<Signal>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for CompletionHandler_Impl {
    fn ActivateCompleted(
        &self,
        _operation: windows::core::Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> Result<()> {
        *self
            .signal
            .completed
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = true;
        self.signal.ready.notify_all();
        Ok(())
    }
}

/// Flux de capture attaché à un processus. La capture s'arrête à la destruction.
pub struct ProcessCapture {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    event: HANDLE,
}

// Les interfaces WASAPI sont utilisees depuis le seul thread qui les a creees.
unsafe impl Send for ProcessCapture {}

impl ProcessCapture {
    pub fn open(process_id: u32) -> Result<Self> {
        let mut activation = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: process_id,
                    // Deezer est une application Electron : le son sort d'un processus fils.
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };

        let variant = BlobPropVariant {
            vt: VT_BLOB,
            reserved1: 0,
            reserved2: 0,
            reserved3: 0,
            size: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            _padding: 0,
            data: std::ptr::from_mut(&mut activation).cast(),
        };

        let signal = Arc::new(Signal::default());
        let handler: IActivateAudioInterfaceCompletionHandler = CompletionHandler {
            signal: signal.clone(),
        }
        .into();

        let operation = unsafe {
            ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                Some(std::ptr::from_ref(&variant).cast::<PROPVARIANT>()),
                &handler,
            )?
        };

        wait_for(&signal)?;

        let mut status = HRESULT(0);
        let mut activated = None;
        unsafe { operation.GetActivateResult(&mut status, &mut activated)? };
        status.ok()?;

        let client: IAudioClient = activated
            .ok_or_else(|| windows::core::Error::from(E_FAIL))?
            .cast()?;

        let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
        let format = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: CHANNELS,
            nSamplesPerSec: SAMPLE_RATE,
            nAvgBytesPerSec: SAMPLE_RATE * u32::from(block_align),
            nBlockAlign: block_align,
            wBitsPerSample: BITS_PER_SAMPLE,
            cbSize: 0,
        };

        unsafe {
            client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                BUFFER_DURATION_HNS,
                0,
                &format,
                None,
            )?;
        }

        // La capture applicative exige un evenement, meme si on interroge le flux nous-memes.
        let event = unsafe { CreateEventW(None, false, false, PCWSTR::null())? };
        unsafe {
            client.SetEventHandle(event)?;
        }

        let capture: IAudioCaptureClient = unsafe { client.GetService()? };
        unsafe { client.Start()? };

        Ok(Self {
            client,
            capture,
            event,
        })
    }

    /// Vide les paquets disponibles, en mixant les canaux en mono.
    pub fn drain(&self, sink: &mut impl FnMut(f32)) -> Result<()> {
        loop {
            let frames = unsafe { self.capture.GetNextPacketSize()? };
            if frames == 0 {
                return Ok(());
            }

            let mut data = std::ptr::null_mut();
            let mut available = 0u32;
            let mut flags = 0u32;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut available, &mut flags, None, None)?;
            }

            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                for _ in 0..available {
                    sink(0.0);
                }
            } else if !data.is_null() {
                let samples = unsafe {
                    std::slice::from_raw_parts(
                        data.cast::<i16>(),
                        available as usize * CHANNELS as usize,
                    )
                };
                for frame in samples.chunks_exact(CHANNELS as usize) {
                    let sum: f32 = frame
                        .iter()
                        .map(|sample| f32::from(*sample) / 32_768.0)
                        .sum();
                    sink(sum / f32::from(CHANNELS));
                }
            }

            unsafe { self.capture.ReleaseBuffer(available)? };
        }
    }
}

impl Drop for ProcessCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = self.client.Stop();
            let _ = CloseHandle(self.event);
        }
    }
}

fn wait_for(signal: &Signal) -> Result<()> {
    let mut completed = signal
        .completed
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    while !*completed {
        let (guard, timeout) = signal
            .ready
            .wait_timeout(completed, ACTIVATION_TIMEOUT)
            .unwrap_or_else(|error| error.into_inner());
        completed = guard;
        if timeout.timed_out() && !*completed {
            return Err(windows::core::Error::from(E_FAIL));
        }
    }

    Ok(())
}
