//! Tests d'intégration : bridge complet + adapter simulé, via HTTP et WebSocket réels.

use std::time::Duration;

use dezzer_bridge::config::{AdapterKind, Config};
use dezzer_bridge::Bridge;
use futures_util::StreamExt;
use serde_json::Value;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct TestBridge {
    base: String,
    data_dir: std::path::PathBuf,
    handle: tokio::task::JoinHandle<()>,
}

impl TestBridge {
    async fn start(label: &str) -> Self {
        let data_dir = std::env::temp_dir().join(format!(
            "dezzer-it-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let config = Config {
            token: TOKEN.to_string(),
            adapter: AdapterKind::Mock,
            port: 0,
            overlay_dir: data_dir.join("overlay"),
            data_dir: data_dir.clone(),
            log_level: "error".into(),
            parent_pid: None,
            dev_mode: true,
        };

        let bridge = Bridge::bind(config).await.expect("demarrage du bridge");
        let base = format!("http://127.0.0.1:{}", bridge.addr.port());
        let handle = tokio::spawn(async move {
            let _ = bridge.serve().await;
        });

        // Laisse l'adapter simule publier son premier instantane.
        tokio::time::sleep(Duration::from_millis(200)).await;

        Self {
            base,
            data_dir,
            handle,
        }
    }

    fn ws_url(&self) -> String {
        self.base.replacen("http://", "ws://", 1)
    }

    async fn get(&self, path: &str, token: Option<&str>) -> reqwest::Response {
        let mut request = reqwest::Client::new().get(format!("{}{path}", self.base));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        request.send().await.expect("requete HTTP")
    }

    async fn post(&self, path: &str, body: Option<Value>) -> reqwest::Response {
        let mut request = reqwest::Client::new()
            .post(format!("{}{path}", self.base))
            .bearer_auth(TOKEN);
        if let Some(body) = body {
            request = request.json(&body);
        }
        request.send().await.expect("requete HTTP")
    }
}

impl Drop for TestBridge {
    fn drop(&mut self) {
        self.handle.abort();
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

#[tokio::test]
async fn health_repond_pret_avec_les_versions_de_contrat() {
    let bridge = TestBridge::start("health").await;

    let response = bridge.get("/health", Some(TOKEN)).await;
    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["ready"], true);
    assert_eq!(body["schemaVersion"], 1);
    assert_eq!(body["adapter"], "mock");
    assert!(body["contractVersion"].is_string());
}

#[tokio::test]
async fn refuse_toute_requete_sans_token_valide() {
    let bridge = TestBridge::start("auth").await;

    assert_eq!(bridge.get("/v1/state", None).await.status(), 401);
    assert_eq!(bridge.get("/v1/state", Some("faux")).await.status(), 401);
    assert_eq!(bridge.get("/health", None).await.status(), 401);
    assert_eq!(bridge.get("/v1/state", Some(TOKEN)).await.status(), 200);
}

#[tokio::test]
async fn n_ajoute_jamais_de_cache_sur_les_reponses_d_etat() {
    let bridge = TestBridge::start("cache").await;

    let response = bridge.get("/v1/state", Some(TOKEN)).await;
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn expose_un_etat_conforme_au_contrat() {
    let bridge = TestBridge::start("state").await;

    let body: Value = bridge
        .get("/v1/state", Some(TOKEN))
        .await
        .json()
        .await
        .unwrap();
    let state = &body["state"];

    assert_eq!(body["ok"], true);
    assert_eq!(state["schemaVersion"], 1);
    assert_eq!(state["source"], "deezer-desktop");
    assert_eq!(state["available"], true);
    assert!(state["sequence"].as_u64().unwrap() >= 1);
    assert!(state["updatedAt"].as_str().unwrap().ends_with('Z'));
    assert_eq!(state["capabilities"]["volume"], false);
}

#[tokio::test]
async fn execute_play_pause_et_renvoie_l_etat_a_jour() {
    let bridge = TestBridge::start("playpause").await;

    let before: Value = bridge
        .get("/v1/state", Some(TOKEN))
        .await
        .json()
        .await
        .unwrap();
    let status_before = before["state"]["status"].as_str().unwrap().to_string();

    let response = bridge.post("/v1/controls/play-pause", None).await;
    assert_eq!(response.status(), 200);

    let after: Value = response.json().await.unwrap();
    assert_eq!(after["ok"], true);
    assert_ne!(after["state"]["status"].as_str().unwrap(), status_before);
}

#[tokio::test]
async fn refuse_une_commande_hors_capacites_avec_un_code_explicite() {
    let bridge = TestBridge::start("caps").await;

    // Le volume n'est jamais disponible : GSMTC ne l'expose pas (matrice M0).
    let response = bridge
        .post(
            "/v1/controls/volume",
            Some(serde_json::json!({ "value": 50 })),
        )
        .await;
    assert_eq!(response.status(), 409);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "UNSUPPORTED_CAPABILITY");
    assert_eq!(body["error"]["retryable"], false);
}

#[tokio::test]
async fn valide_les_corps_de_requete_de_controle() {
    let bridge = TestBridge::start("validation").await;

    for body in [
        serde_json::json!({ "value": 900 }),
        serde_json::json!({ "value": -5 }),
        serde_json::json!({ "autre": 1 }),
    ] {
        assert_eq!(
            bridge
                .post("/v1/controls/volume", Some(body))
                .await
                .status(),
            400
        );
    }

    assert_eq!(
        bridge
            .post(
                "/v1/controls/seek",
                Some(serde_json::json!({ "positionMs": -1 }))
            )
            .await
            .status(),
        400
    );
}

#[tokio::test]
async fn borne_le_seek_a_la_duree_annoncee() {
    let bridge = TestBridge::start("seek").await;

    let response = bridge
        .post(
            "/v1/controls/seek",
            Some(serde_json::json!({ "positionMs": 999_999_999u64 })),
        )
        .await;
    assert_eq!(response.status(), 200);

    // Une demande aberrante ne doit jamais produire une position hors piste. Le lecteur
    // simule enchaine sur la piste suivante des qu'il atteint la fin, d'ou la comparaison
    // avec la duree courante et non avec la duree d'origine.
    let body: Value = response.json().await.unwrap();
    let position = body["state"]["positionMs"].as_u64().unwrap();
    let duration = body["state"]["durationMs"].as_u64().unwrap();
    assert!(
        position <= duration,
        "position {position} hors de la piste ({duration} ms)"
    );

    let response = bridge
        .post(
            "/v1/controls/seek",
            Some(serde_json::json!({ "positionMs": 42_000u64 })),
        )
        .await;
    let body: Value = response.json().await.unwrap();
    let position = body["state"]["positionMs"].as_u64().unwrap();
    assert!(
        (42_000..43_000).contains(&position),
        "le seek doit atteindre la position demandee, obtenu {position}"
    );
}

#[tokio::test]
async fn echappe_les_metadonnees_hostiles_dans_la_reponse_json() {
    let bridge = TestBridge::start("xss").await;

    // La piste 3 du mock contient une charge XSS.
    for _ in 0..2 {
        bridge.post("/v1/controls/next", None).await;
    }

    let raw = bridge
        .get("/v1/state", Some(TOKEN))
        .await
        .text()
        .await
        .unwrap();

    assert!(
        raw.contains("\\u003cscript\\u003e") || raw.contains("<script>"),
        "le titre hostile doit etre transporte tel quel dans du JSON, jamais interprete"
    );
    let body: Value = serde_json::from_str(&raw).unwrap();
    assert!(body["state"]["title"]
        .as_str()
        .unwrap()
        .contains("<script>"));
}

#[tokio::test]
async fn sert_la_pochette_depuis_une_url_locale() {
    let bridge = TestBridge::start("artwork").await;

    let body: Value = bridge
        .get("/v1/state", Some(TOKEN))
        .await
        .json()
        .await
        .unwrap();
    let url = body["state"]["artworkUrl"].as_str().expect("pochette");
    assert!(url.starts_with("/v1/artwork/"));

    let response = bridge.get(url, Some(TOKEN)).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );
    assert!(!response.bytes().await.unwrap().is_empty());
}

#[tokio::test]
async fn refuse_une_cle_de_pochette_traversant_le_systeme_de_fichiers() {
    let bridge = TestBridge::start("traversal").await;

    for key in ["..%2f..%2fetc%2fpasswd", "a/b", "..", "%2e%2e"] {
        let status = bridge
            .get(&format!("/v1/artwork/{key}"), Some(TOKEN))
            .await
            .status();
        assert!(
            status == 400 || status == 404,
            "cle `{key}` acceptee a tort (status {status})"
        );
    }
}

#[tokio::test]
async fn le_websocket_envoie_l_etat_immediatement_puis_les_changements() {
    let bridge = TestBridge::start("ws").await;

    let url = format!("{}/v1/events?token={TOKEN}", bridge.ws_url());
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("connexion websocket");

    let first = next_event(&mut socket).await;
    assert_eq!(first["type"], "playback.state");
    assert_eq!(first["payload"]["schemaVersion"], 1);
    let first_sequence = first["payload"]["sequence"].as_u64().unwrap();

    bridge.post("/v1/controls/next", None).await;

    let mut saw_newer = false;
    for _ in 0..10 {
        let event = next_event(&mut socket).await;
        if event["type"] == "playback.state"
            && event["payload"]["sequence"].as_u64().unwrap() > first_sequence
        {
            saw_newer = true;
            break;
        }
    }
    assert!(saw_newer, "un changement de piste doit etre diffuse");
}

#[tokio::test]
async fn le_websocket_refuse_une_connexion_sans_token() {
    let bridge = TestBridge::start("ws-auth").await;

    let url = format!("{}/v1/events", bridge.ws_url());
    assert!(
        tokio_tungstenite::connect_async(url).await.is_err(),
        "une connexion sans token doit etre rejetee"
    );

    let url = format!("{}/v1/events?token=faux", bridge.ws_url());
    assert!(tokio_tungstenite::connect_async(url).await.is_err());
}

#[tokio::test]
async fn le_fichier_de_disponibilite_expose_le_port_sans_le_token() {
    let bridge = TestBridge::start("runtime").await;

    let path = dezzer_bridge::runtime::RuntimeFile::path(&bridge.data_dir);
    let contents = std::fs::read_to_string(&path).expect("fichier de disponibilite");
    let info: Value = serde_json::from_str(&contents).unwrap();

    assert!(info["port"].as_u64().unwrap() > 0);
    assert_eq!(info["pid"].as_u64().unwrap(), std::process::id() as u64);
    assert!(!contents.to_lowercase().contains("token"));
    assert!(!contents.contains(TOKEN));
}

async fn next_event<S>(socket: &mut S) -> Value
where
    S: StreamExt<
            Item = Result<
                tokio_tungstenite::tungstenite::Message,
                tokio_tungstenite::tungstenite::Error,
            >,
        > + Unpin,
{
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("timeout websocket")
            .expect("flux ferme")
            .expect("erreur websocket");

        if let tokio_tungstenite::tungstenite::Message::Text(text) = message {
            return serde_json::from_str(&text).expect("json invalide");
        }
    }
}
