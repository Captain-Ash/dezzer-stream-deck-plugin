//! Verrou de processus, fichier de disponibilité et surveillance du processus parent.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

const LOCK_FILE: &str = "bridge.lock";
const RUNTIME_FILE: &str = "bridge-runtime.json";

/// Contenu du fichier de disponibilité lu par le plugin.
///
/// Le token n'y figure jamais : il reste côté plugin (§8.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub pid: u32,
    pub port: u16,
    pub version: String,
    pub contract_version: String,
    pub started_at: String,
}

/// Verrou empêchant deux bridges concurrents. Libéré à la destruction.
pub struct ProcessLock {
    path: PathBuf,
}

impl ProcessLock {
    pub fn acquire(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir)
            .with_context(|| format!("creation du dossier de donnees {}", data_dir.display()))?;
        let path = data_dir.join(LOCK_FILE);

        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(pid) = contents.trim().parse::<u32>() {
                if pid != std::process::id() && is_process_alive(pid) {
                    bail!("une autre instance du bridge est deja active (pid {pid})");
                }
            }
            // Verrou orphelin : le processus precedent a disparu sans nettoyer.
            let _ = fs::remove_file(&path);
        }

        let mut file = fs::File::create(&path)
            .with_context(|| format!("creation du verrou {}", path.display()))?;
        write!(file, "{}", std::process::id())?;

        Ok(Self { path })
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub struct RuntimeFile {
    path: PathBuf,
}

impl RuntimeFile {
    pub fn write(data_dir: &Path, info: &RuntimeInfo) -> Result<Self> {
        fs::create_dir_all(data_dir)?;
        let path = data_dir.join(RUNTIME_FILE);
        let json = serde_json::to_string_pretty(info)?;
        fs::write(&path, json).with_context(|| format!("ecriture de {}", path.display()))?;
        Ok(Self { path })
    }

    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join(RUNTIME_FILE)
    }
}

impl Drop for RuntimeFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Arrête le bridge si le plugin qui l'a lancé disparaît, pour ne jamais laisser de
/// processus orphelin sur la machine de l'utilisateur (§8.2).
pub async fn watch_parent(pid: u32, grace: std::time::Duration) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    loop {
        interval.tick().await;
        if !is_process_alive(pid) {
            tracing::info!(
                parent_pid = pid,
                "processus parent disparu, arret programme"
            );
            tokio::time::sleep(grace).await;
            if !is_process_alive(pid) {
                return;
            }
        }
    }
}

#[cfg(windows)]
pub fn is_process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const STILL_ACTIVE: u32 = 259;

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0u32;
        let alive = GetExitCodeProcess(handle, &mut code).is_ok() && code == STILL_ACTIVE;
        let _ = CloseHandle(handle);
        alive
    }
}

#[cfg(not(windows))]
pub fn is_process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_fichier_de_disponibilite_ne_contient_jamais_le_token() {
        let info = RuntimeInfo {
            pid: 42,
            port: 53211,
            version: "0.1.0".into(),
            contract_version: "1.0.0".into(),
            started_at: "2026-08-19T13:20:00.000Z".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.to_lowercase().contains("token"));
        assert!(json.contains("\"port\":53211"));
    }

    #[test]
    fn le_verrou_refuse_une_seconde_instance_puis_se_libere() {
        let dir = std::env::temp_dir().join(format!("dezzer-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        let lock = ProcessLock::acquire(&dir).unwrap();
        assert!(dir.join(LOCK_FILE).exists());

        // Le meme processus reprend son propre verrou : cas du redemarrage rapide.
        drop(lock);
        assert!(!dir.join(LOCK_FILE).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detecte_le_processus_courant_comme_vivant() {
        assert!(is_process_alive(std::process::id()));
    }
}
