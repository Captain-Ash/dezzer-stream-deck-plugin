//! Configuration du bridge, exclusivement issue de variables d'environnement.
//!
//! Le token n'est jamais passé en argument de ligne de commande : `argv` est lisible par
//! les autres processus de la machine, contrairement à l'environnement d'un processus.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

pub const ENV_TOKEN: &str = "DEZZER_BRIDGE_TOKEN";
pub const ENV_ADAPTER: &str = "DEZZER_BRIDGE_ADAPTER";
pub const ENV_PORT: &str = "DEZZER_BRIDGE_PORT";
pub const ENV_OVERLAY_DIR: &str = "DEZZER_BRIDGE_OVERLAY_DIR";
pub const ENV_DATA_DIR: &str = "DEZZER_BRIDGE_DATA_DIR";
pub const ENV_LOG_LEVEL: &str = "DEZZER_BRIDGE_LOG_LEVEL";
pub const ENV_PARENT_PID: &str = "DEZZER_BRIDGE_PARENT_PID";

/// Port fixe par défaut.
///
/// Un port éphémère obligerait à recoller l'URL dans OBS à chaque redémarrage. Cette
/// valeur est hors de la plage éphémère de Windows (49152-65535), hors des plages
/// exclues par HTTP.sys, et non attribuée par l'IANA.
pub const DEFAULT_PORT: u16 = 39_217;

/// Ports essayés successivement si le port par défaut est pris. Le dernier recours est un
/// port éphémère : le bridge reste utilisable, au prix d'une URL d'overlay à recoller.
pub const PORT_FALLBACK_ATTEMPTS: u16 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    /// Choisit l'adapter natif de la plateforme, avec repli sur l'adapter indisponible.
    Auto,
    Windows,
    Mock,
}

impl AdapterKind {
    fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "windows" | "win" | "wmc" => Ok(Self::Windows),
            "mock" => Ok(Self::Mock),
            other => bail!("adapter inconnu : {other}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub adapter: AdapterKind,
    /// Port souhaité. `0` demande explicitement un port éphémère.
    pub port: u16,
    pub overlay_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_level: String,
    /// Si renseigné, le bridge s'arrête peu après la disparition de ce processus.
    pub parent_pid: Option<u32>,
    /// Mode développement : token auto-généré et affiché sur stdout.
    pub dev_mode: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let dev_mode = std::env::args().any(|a| a == "--dev");

        let token = match std::env::var(ENV_TOKEN) {
            Ok(value) if !value.trim().is_empty() => value,
            _ if dev_mode => generate_token(),
            _ => bail!(
                "{ENV_TOKEN} est requis. Le bridge n'est pas prévu pour un lancement manuel ; \
                 utilisez --dev pour un token de développement."
            ),
        };

        if token.len() < 32 {
            bail!("{ENV_TOKEN} doit contenir au moins 32 caractères");
        }

        let adapter = AdapterKind::parse(&std::env::var(ENV_ADAPTER).unwrap_or_default())?;

        let port = match std::env::var(ENV_PORT) {
            Ok(value) if !value.trim().is_empty() => value
                .trim()
                .parse::<u16>()
                .with_context(|| format!("{ENV_PORT} invalide : {value}"))?,
            _ => DEFAULT_PORT,
        };

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));

        let overlay_dir = std::env::var_os(ENV_OVERLAY_DIR)
            .map(PathBuf::from)
            .unwrap_or_else(|| exe_dir.join("overlay"));

        let data_dir = std::env::var_os(ENV_DATA_DIR)
            .map(PathBuf::from)
            .unwrap_or_else(default_data_dir);

        let log_level = std::env::var(ENV_LOG_LEVEL)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "info".to_string());

        let parent_pid = std::env::var(ENV_PARENT_PID)
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|pid| *pid != 0);

        Ok(Self {
            token,
            adapter,
            port,
            overlay_dir,
            data_dir,
            log_level,
            parent_pid,
            dev_mode,
        })
    }
}

pub fn default_data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("Dezzer")
}

pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genere_un_token_de_256_bits_en_hexadecimal() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(token, generate_token());
    }

    #[test]
    fn parse_les_noms_d_adapter_acceptes() {
        assert_eq!(AdapterKind::parse("").unwrap(), AdapterKind::Auto);
        assert_eq!(AdapterKind::parse(" Mock ").unwrap(), AdapterKind::Mock);
        assert_eq!(AdapterKind::parse("WINDOWS").unwrap(), AdapterKind::Windows);
        assert!(AdapterKind::parse("spotify").is_err());
    }

    #[test]
    fn le_port_par_defaut_evite_la_plage_ephemere_de_windows() {
        assert!(
            DEFAULT_PORT < 49_152,
            "un port dans la plage ephemere pourrait etre pris par un autre programme"
        );
        assert!(
            DEFAULT_PORT > 1_024,
            "un port privilegie exigerait des droits admin"
        );
    }
}
