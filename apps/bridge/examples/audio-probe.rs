//! Spike : le volume par application est-il pilotable pour Deezer Desktop ?
//!
//! L'API de sessions média (GSMTC) n'expose aucun volume. La seule voie publique est
//! Windows Core Audio, qui pilote le curseur de Deezer dans le mixeur Windows.
//!
//! Lecture seule par défaut. `cargo run --example audio-probe -- --set 40` applique
//! réellement un volume, puis restaure la valeur d'origine.

use windows::core::Interface;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, AudioSessionStateActive, AudioSessionStateExpired,
    AudioSessionStateInactive, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
fn main() -> windows::core::Result<()> {
    let target_volume: Option<f32> = std::env::args()
        .skip_while(|a| a != "--set")
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v / 100.0).clamp(0.0, 1.0));

    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let sessions = manager.GetSessionEnumerator()?;
        let count = sessions.GetCount()?;

        println!("sessions audio du peripherique de sortie : {count}\n");

        let mut deezer: Vec<(u32, ISimpleAudioVolume)> = Vec::new();

        for index in 0..count {
            let control = sessions.GetSession(index)?;
            let control2: IAudioSessionControl2 = control.cast()?;

            let pid = control2.GetProcessId().unwrap_or(0);
            let state = match control.GetState() {
                Ok(value) if value == AudioSessionStateActive => "active",
                Ok(value) if value == AudioSessionStateInactive => "inactive",
                Ok(value) if value == AudioSessionStateExpired => "expiree",
                _ => "?",
            };
            let identifier = control2
                .GetSessionIdentifier()
                .map(|s| s.to_string().unwrap_or_default())
                .unwrap_or_default();
            // `IsSystemSoundsSession` renvoie S_FALSE, que windows-rs considere comme un
            // succes : le PID nul est le seul critere fiable.
            let is_system = pid == 0;
            let name = process_name(pid);

            let volume: ISimpleAudioVolume = control.cast()?;
            let level = volume.GetMasterVolume().unwrap_or(-1.0);
            let muted = volume.GetMute().map(|m| m.as_bool()).unwrap_or(false);

            let deezer_like = name.to_lowercase().contains("deezer")
                || identifier.to_lowercase().contains("deezer");

            println!(
                "{marker} pid={pid:<6} {name:<24} etat={state:<9} volume={pct:>5.1}% muet={muted} systeme={is_system}",
                marker = if deezer_like { ">>" } else { "  " },
                pct = level * 100.0,
            );
            if !identifier.is_empty() {
                println!("     id: {identifier}");
            }

            if deezer_like {
                deezer.push((pid, volume));
            }
        }

        println!();
        if deezer.is_empty() {
            println!("Aucune session audio Deezer : le volume par application est indisponible.");
            return Ok(());
        }

        println!("Sessions Deezer trouvees : {}", deezer.len());

        if let Some(target) = target_volume {
            let (pid, volume) = &deezer[0];
            let before = volume.GetMasterVolume()?;
            println!(
                "\ntest d'ecriture sur pid={pid} : {:.1}% -> {:.1}%",
                before * 100.0,
                target * 100.0
            );
            volume.SetMasterVolume(target, std::ptr::null())?;
            std::thread::sleep(std::time::Duration::from_millis(1200));
            let after = volume.GetMasterVolume()?;
            println!("relu : {:.1}%", after * 100.0);
            volume.SetMasterVolume(before, std::ptr::null())?;
            println!("restaure a {:.1}%", before * 100.0);
        } else {
            println!("(relancez avec `-- --set 40` pour tester l'ecriture)");
        }
    }

    Ok(())
}

fn process_name(pid: u32) -> String {
    if pid == 0 {
        return "<sons systeme>".into();
    }
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return format!("<pid {pid}>");
        };
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let name = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            String::from_utf16_lossy(&buffer[..size as usize])
                .rsplit('\\')
                .next()
                .unwrap_or_default()
                .to_string()
        } else {
            format!("<pid {pid}>")
        };
        let _ = CloseHandle(handle);
        name
    }
}
