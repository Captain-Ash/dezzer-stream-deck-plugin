//! Contrôle du volume par application, via Windows Core Audio.
//!
//! Les sessions média (GSMTC) n'exposent aucun volume. On pilote donc le curseur de Deezer
//! dans le mixeur Windows, exactement comme le fait le mixeur du système : API publique,
//! aucune injection, aucune automatisation d'interface.
//!
//! Conséquence assumée : ce n'est pas le curseur interne de Deezer. Les deux se multiplient.

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

/// Nom de l'exécutable qui détient la session audio de Deezer Desktop.
const DEEZER_EXECUTABLE: &str = "deezer.exe";

/// Une session audio n'apparaît qu'une fois que l'application a produit du son, et son PID
/// change à chaque redémarrage de Deezer : la session est donc relue à chaque appel.
pub struct AppVolume;

impl AppVolume {
    /// Volume courant de Deezer dans le mixeur Windows, de 0 à 100.
    pub fn get() -> Option<u8> {
        let session = find_deezer_session::<ISimpleAudioVolume>()?;
        let level = unsafe { session.GetMasterVolume() }.ok()?;
        let muted = unsafe { session.GetMute() }
            .map(|m| m.as_bool())
            .unwrap_or(false);
        Some(if muted {
            0
        } else {
            (level * 100.0).round().clamp(0.0, 100.0) as u8
        })
    }

    pub fn set(value: u8) -> windows::core::Result<()> {
        let Some(session) = find_deezer_session::<ISimpleAudioVolume>() else {
            return Err(windows::core::Error::from(
                windows::Win32::Foundation::E_FAIL,
            ));
        };
        unsafe {
            // Sortir du silence explicitement : un volume relevé sur une session muette
            // resterait inaudible.
            if value > 0 {
                let _ = session.SetMute(false, std::ptr::null());
            }
            session.SetMasterVolume(f32::from(value) / 100.0, std::ptr::null())
        }
    }

    pub fn is_available() -> bool {
        find_deezer_session::<ISimpleAudioVolume>().is_some()
    }
}

/// Identifiant du processus Deezer qui produit le son, pour la capture applicative.
pub fn deezer_process_id() -> Option<u32> {
    find_deezer_control().map(|(pid, _)| pid)
}

fn find_deezer_session<T: Interface>() -> Option<T> {
    find_deezer_control().and_then(|(_, control)| control.cast::<T>().ok())
}

fn find_deezer_control() -> Option<(u32, IAudioSessionControl)> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None).ok()?;
        let sessions = manager.GetSessionEnumerator().ok()?;
        let count = sessions.GetCount().ok()?;

        for index in 0..count {
            let Ok(control) = sessions.GetSession(index) else {
                continue;
            };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let pid = control2.GetProcessId().unwrap_or(0);
            if pid == 0 || !is_deezer_process(pid) {
                continue;
            }
            return Some((pid, control));
        }
        None
    }
}

fn is_deezer_process(pid: u32) -> bool {
    process_executable(pid)
        .map(|name| name.eq_ignore_ascii_case(DEEZER_EXECUTABLE))
        .unwrap_or(false)
}

fn process_executable(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);

        result.ok()?;
        String::from_utf16_lossy(&buffer[..size as usize])
            .rsplit('\\')
            .next()
            .map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ne_reconnait_que_l_executable_de_deezer() {
        assert!(DEEZER_EXECUTABLE.eq_ignore_ascii_case("Deezer.exe"));
        assert!(!DEEZER_EXECUTABLE.eq_ignore_ascii_case("Spotify.exe"));
    }
}
