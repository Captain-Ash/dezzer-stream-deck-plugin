import { effectivePositionMs, formatDuration } from "@dezzer/playback-contract";
import streamDeck, { action, type KeyDownEvent } from "@elgato/streamdeck";

import type { BridgeSnapshot } from "../bridge-service.js";
import { t } from "../i18n.js";
import {
  estimateTextWidth,
  glyphKey,
  nowPlayingKey,
  unavailableKey,
  type Artwork,
} from "../key-render.js";
import {
  buildOverlayUrl,
  normaliseOverlay,
  type GlobalSettings,
  type NowPlayingFormat,
} from "../settings.js";
import { BridgeAction, unavailableLabel, type AnyAction } from "./base.js";

/** Au-delà, le texte déborde de la touche et doit défiler. */
const KEY_TEXT_VIEWPORT = 128;

@action({ UUID: "com.dezzer.deezer.now-playing" })
export class NowPlayingAction extends BridgeAction {
  private scrollStartedAtMs = Date.now();
  private lastTrackId: string | undefined;
  private scrolling = false;

  /** Le défilement n'est animé que lorsqu'un texte déborde réellement. */
  override get needsAnimation(): boolean {
    return this.scrolling;
  }

  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    // Un appui bascule la lecture : c'est le geste attendu sur une touche « en cours ».
    const failure = await this.service.command("play-pause");
    if (failure) {
      await event.action.showAlert();
      await this.flash(event.action, failure);
    }
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const label = unavailableLabel(snapshot);
    if (label) {
      this.scrolling = false;
      await action.setImage(unavailableKey("overlay", t(label)));
      await action.setTitle("");
      return;
    }

    const { state } = snapshot;
    if (state.trackId !== this.lastTrackId) {
      this.lastTrackId = state.trackId;
      this.scrollStartedAtMs = Date.now();
    }

    const settings = await this.service.globalSettings();
    const format: NowPlayingFormat = settings?.nowPlayingFormat ?? "title-artist";

    const title = format === "artist" ? "" : (state.title ?? "");
    const artist = format === "title" ? "" : (state.artist ?? "");
    const position = effectivePositionMs(state, snapshot.receivedAtMs);

    this.scrolling =
      estimateTextWidth(title, 19, true) > KEY_TEXT_VIEWPORT ||
      estimateTextWidth(artist, 15) > KEY_TEXT_VIEWPORT;

    const artwork: Artwork | undefined = await this.artwork(snapshot);

    await action.setImage(
      nowPlayingKey({
        title,
        artist,
        time: position === undefined ? undefined : formatDuration(position),
        playing: state.status === "playing",
        artwork,
        elapsedMs: Date.now() - this.scrollStartedAtMs,
      }),
    );
    // Le texte est dessine dans l'image : le titre natif resterait superpose.
    await action.setTitle("");
  }
}

@action({ UUID: "com.dezzer.deezer.diagnostics" })
export class DiagnosticsAction extends BridgeAction {
  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    await this.service.restart();
    await this.flash(event.action, t("key.restarting"));
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const { mood, titleKey } = diagnose(snapshot);
    await action.setImage(glyphKey({ glyph: "diagnostics", mood }));
    await action.setTitle(t(titleKey));
  }
}

/**
 * État du service, sous forme de clés de traduction : la fonction reste pure et sert
 * aussi bien la touche que le Property Inspector.
 */
export function diagnose(snapshot: BridgeSnapshot): {
  mood: "active" | "idle" | "disabled" | "error";
  titleKey: string;
  messageKey: string;
  detail?: string;
} {
  switch (snapshot.status) {
    case "starting":
      return {
        mood: "idle",
        titleKey: "key.starting",
        messageKey: "diagnostic.starting",
      };
    case "stopped":
      return {
        mood: "disabled",
        titleKey: "key.bridgeOff",
        messageKey: "diagnostic.bridgeStopped",
      };
    case "failed":
      return {
        mood: "error",
        titleKey: "key.bridgeError",
        messageKey: "diagnostic.bridgeFailed",
        detail: snapshot.lastError,
      };
    case "ready":
      if (!snapshot.state.available) {
        return {
          mood: "disabled",
          titleKey: "key.deezerOff",
          messageKey: "diagnostic.deezerOff",
        };
      }
      if (snapshot.state.status === "playing") {
        return {
          mood: "active",
          titleKey: "key.deezerPlaying",
          messageKey: "diagnostic.playing",
        };
      }
      return { mood: "idle", titleKey: "key.deezerReady", messageKey: "diagnostic.paused" };
  }
}

@action({ UUID: "com.dezzer.deezer.overlay-info" })
export class OverlayInfoAction extends BridgeAction {
  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    const snapshot = this.service.snapshot();
    if (!snapshot.info) {
      await event.action.showAlert();
      await this.flash(event.action, t("key.bridgeOff"));
      return;
    }

    const settings = await streamDeck.settings.getGlobalSettings<GlobalSettings>();
    const url = buildOverlayUrl(
      snapshot.info.port,
      snapshot.info.token,
      normaliseOverlay(settings?.overlay),
    );

    // L'URL contient le jeton : elle part vers le navigateur local, jamais vers un log.
    streamDeck.system.openUrl(url);
    await event.action.showOk();
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    if (!snapshot.info) {
      await action.setImage(unavailableKey("overlay", t("key.bridgeOff")));
      await action.setTitle("");
      return;
    }

    await action.setImage(glyphKey({ glyph: "overlay", mood: "idle" }));
    await action.setTitle(t("key.overlay"));
  }
}
