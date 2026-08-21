/** Point d'entrée du plugin Deezer. */

import { randomBytes } from "node:crypto";
import { dirname, resolve } from "node:path";

import streamDeck from "@elgato/streamdeck";

import type { BridgeAction } from "./actions/base.js";
import { DiagnosticsAction, NowPlayingAction, diagnose } from "./actions/info.js";
import {
  NextAction,
  PlayPauseAction,
  PreviousAction,
  VolumeDownAction,
  VolumeUpAction,
} from "./actions/transport.js";
import { BridgeService } from "./bridge-service.js";
import { t } from "./i18n.js";
import { type GlobalSettings } from "./settings.js";

/** Cadence de rafraîchissement du temps écoulé sur la touche « Morceau en cours ». */
const TICK_MS = 1_000;

/** Cadence d'animation du défilement, uniquement quand un texte déborde. */
const ANIMATION_TICK_MS = 125;

/**
 * Libellés du Property Inspector.
 *
 * Il tourne dans un navigateur et n'a donc pas accès aux fichiers de langue : le plugin,
 * seul à connaître la langue retenue par Stream Deck, les lui transmet en bloc.
 */
const INSPECTOR_KEYS = [
  "pi.status",
  "pi.localService",
  "pi.deezer",
  "pi.session",
  "pi.capabilities",
  "pi.testConnection",
  "pi.restartService",
  "pi.keys",
  "pi.nowPlayingText",
  "pi.formatTitleArtist",
  "pi.formatTitle",
  "pi.formatArtist",
  "pi.artworkOnKeys",
  "pi.volumeStep",
  "pi.volumeHint",
  "pi.rotateToken",
  "pi.rotateConfirm",
  "pi.diagnosticExport",
  "pi.copyDiagnostic",
  "pi.diagnosticHint",
  "pi.bridgeReady",
  "pi.bridgeStarting",
  "pi.bridgeStopped",
  "pi.bridgeFailed",
  "pi.playerPlaying",
  "pi.playerPaused",
  "pi.playerMissing",
  "pi.serviceUnavailable",
  "pi.capPlayPause",
  "pi.capNext",
  "pi.capPrevious",
  "pi.capStop",
  "pi.capSeek",
  "pi.capVolume",
];

async function main(): Promise<void> {
  streamDeck.logger.setLevel("info");

  const pluginRoot = resolve(dirname(process.argv[1] ?? process.cwd()), "..");
  const service = new BridgeService(pluginRoot, ensureToken);

  const actions: BridgeAction[] = [
    new PlayPauseAction(service),
    new NextAction(service),
    new PreviousAction(service),
    new VolumeUpAction(service),
    new VolumeDownAction(service),
    new NowPlayingAction(service),
    new DiagnosticsAction(service),
  ];

  for (const action of actions) {
    streamDeck.actions.registerAction(action);
  }

  service.subscribe((snapshot) => {
    for (const action of actions) action.refresh(snapshot);
    void publishInspectorState(service);
  });

  // Le bridge ne rediffuse pas la position a chaque seconde : on l'extrapole ici.
  setInterval(() => {
    const snapshot = service.snapshot();
    if (snapshot.state.status === "playing") {
      for (const action of actions) action.refresh(snapshot);
    }
  }, TICK_MS);

  // Defilement des textes trop longs. On ne reemet une image que si une touche concernee
  // est visible et que son texte deborde reellement.
  setInterval(() => {
    const animated = actions.filter((action) => action.needsAnimation && action.hasVisibleActions());
    if (animated.length === 0) return;
    const snapshot = service.snapshot();
    for (const action of animated) action.refresh(snapshot);
  }, ANIMATION_TICK_MS);

  streamDeck.ui.onSendToPlugin(async (event) => {
    const payload = event.payload as { action?: string } | undefined;

    switch (payload?.action) {
      case "getState":
        await publishInspectorState(service);
        break;
      case "getTranslations":
        await streamDeck.ui.sendToPropertyInspector({
          event: "dezzer.i18n",
          translations: inspectorTranslations(),
        });
        break;
      case "restartBridge":
        await service.restart();
        await publishInspectorState(service);
        break;
      case "rotateToken": {
        const settings = await streamDeck.settings.getGlobalSettings<GlobalSettings>();
        await streamDeck.settings.setGlobalSettings<GlobalSettings>({
          ...settings,
          token: randomBytes(32).toString("hex"),
        });
        // Le bridge ne connait que l'ancien jeton : il doit repartir avec le nouveau.
        await service.restart();
        await publishInspectorState(service);
        break;
      }
      default:
        break;
    }
  });

  // Aucune lecture de reglages avant cette ligne : tant que la connexion n'est pas
  // etablie, les promesses de `streamDeck.settings` ne se resolvent jamais et le
  // processus s'arreterait silencieusement.
  await streamDeck.connect();
  await service.ensureRunning();

  const shutdown = () => {
    void service.dispose().finally(() => process.exit(0));
  };
  process.once("SIGINT", shutdown);
  process.once("SIGTERM", shutdown);
  process.once("beforeExit", () => void service.dispose());
}

/**
 * Le jeton est propre à l'installation et persistant : le service local doit rester
 * joignable après un redémarrage de Stream Deck.
 */
async function ensureToken(): Promise<string> {
  const settings = await streamDeck.settings.getGlobalSettings<GlobalSettings>();
  if (typeof settings?.token === "string" && settings.token.length >= 64) {
    return settings.token;
  }

  const generated = randomBytes(32).toString("hex");
  await streamDeck.settings.setGlobalSettings<GlobalSettings>({ ...settings, token: generated });
  return generated;
}

/** Diagnostic lisible : le détail technique du bridge complète le message traduit. */
function diagnosticMessage(snapshot: ReturnType<BridgeService["snapshot"]>): string {
  const { messageKey, detail } = diagnose(snapshot);
  return detail ? `${t(messageKey)} ${detail}` : t(messageKey);
}

/**
 * Le SDK renvoie la clé elle-même quand elle est introuvable. On ne transmet donc que les
 * clés réellement traduites, pour que le Property Inspector garde son texte de repli.
 */
function inspectorTranslations(): Record<string, string> {
  const resolved: Record<string, string> = {};
  for (const key of INSPECTOR_KEYS) {
    const value = t(key);
    if (value !== key) resolved[key] = value;
  }
  return resolved;
}

/** Alimente le Property Inspector. Aucun jeton n'y figure. */
async function publishInspectorState(service: BridgeService): Promise<void> {
  if (!streamDeck.ui.action) return;

  const snapshot = service.snapshot();

  await streamDeck.ui.sendToPropertyInspector({
    event: "dezzer.state",
    bridge: {
      status: snapshot.status,
      version: snapshot.info?.version,
      contractVersion: snapshot.info?.contractVersion,
      adapter: snapshot.info?.adapter,
      port: snapshot.info?.port,
      lastError: snapshot.lastError,
    },
    player: {
      available: snapshot.state.available,
      status: snapshot.state.status,
      sourceLabel: snapshot.state.sourceLabel,
      title: snapshot.state.title,
      artist: snapshot.state.artist,
      capabilities: snapshot.state.capabilities,
    },
    diagnostic: diagnosticMessage(snapshot),
    environment: {
      pluginVersion: streamDeck.info.plugin.version,
      platform: process.platform,
      arch: process.arch,
    },
  });
}
void main().catch((error) => {
  streamDeck.logger.error("demarrage du plugin impossible", error);
  process.exit(1);
});
