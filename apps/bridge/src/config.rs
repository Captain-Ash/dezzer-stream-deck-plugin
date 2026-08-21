//! Configuration du bridge, exclusivement issue de variables d'environnement.
//!
//! Le token n'est jamais passé en argument de ligne de commande : `argv` est lisible par
//! les autres processus de la machine, contrairement à l'environnement d'un processus.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

pub const ENV_TOKEN: &str = "DEEZER_BRIDGE_TOKEN";
pub const ENV_ADAPTER: &str = "DEEZER_BRIDGE_ADAPTER";
pub const ENV_PORT: &str = "DEEZER_BRIDGE_PORT";
pub const ENV_DATA_DIR: &str = "DEEZER_BRIDGE_DATA_DIR";
pub const ENV_LOG_LEVEL: &str = "DEEZER_BRIDGE_LOG_LEVEL";
pub const ENV_PARENT_PID: &str = "DEEZER_BRIDGE_PARENT_PID";

/// Port par défaut : `0`, c'est-à-dire un port éphémère attribué par le système.
///
/// Le plugin lit le port réel dans le fichier de disponibilité : rien ne justifie de
/// réserver un numéro fixe, qui risquerait au contraire d'entrer en conflit avec un autre
/// programme déjà installé sur la machine.
pub const DEFAULT_PORT: u16 = 0;

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
    /// Port souhaité. `0` demande un port éphémère.
    pub port: u16,
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
    base.join("Deezer")
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
    fn le_port_par_defaut_est_ephemere() {
        assert_eq!(
            DEFAULT_PORT, 0,
            "le port doit etre attribue par le systeme : le plugin le lit dans le fichier de disponibilite"
        );
    }
}
