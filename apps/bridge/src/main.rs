//! Binaire du bridge Dezzer. Aucun affichage, aucune fenêtre : il est piloté par le plugin.

// Empeche l'ouverture d'une console lorsque le plugin lance le binaire (§8.1).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use dezzer_bridge::{config::Config, logging, run};

fn main() -> std::process::ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("dezzer-bridge: {error:#}");
            return std::process::ExitCode::from(2);
        }
    };

    let _logging = logging::init(
        &config.data_dir.join("logs"),
        &config.log_level,
        config.dev_mode,
    );

    if config.dev_mode {
        // Uniquement en developpement : le token n'est jamais journalise en production.
        println!("DEZZER_BRIDGE_TOKEN={}", config.token);
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "impossible de demarrer le runtime asynchrone");
            return std::process::ExitCode::from(3);
        }
    };

    match runtime.block_on(run(config)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(error = format!("{error:#}"), "arret sur erreur");
            eprintln!("dezzer-bridge: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
