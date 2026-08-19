/**
 * Point d'accès unique du plugin au bridge : supervision, état courant et commandes.
 *
 * Aucune action ne parle directement à l'OS ni ne duplique la logique de connexion.
 */

import {
  UNAVAILABLE_STATE,
  isNewerState,
  type NowPlayingState,
} from "@dezzer/playback-contract";
import streamDeck from "@elgato/streamdeck";
import WebSocket from "ws";

import { BridgeManager, type BridgeConnectionInfo, type BridgeStatus } from "./bridge-manager.js";
import { t } from "./i18n.js";
import type { GlobalSettings } from "./settings.js";

export type CommandName = "play-pause" | "next" | "previous" | "stop";

export interface BridgeSnapshot {
  status: BridgeStatus;
  state: NowPlayingState;
  /** Horodatage local de réception, nécessaire pour extrapoler la position. */
  receivedAtMs: number;
  info: BridgeConnectionInfo | undefined;
  lastError: string | undefined;
}

const WS_RECONNECT_DELAYS_MS = [500, 1_000, 2_000, 5_000, 10_000];

/** Pochettes conservées en mémoire pour les images de touche. */
const ARTWORK_CACHE_SIZE = 4;
const MAX_ARTWORK_BYTES = 2 * 1024 * 1024;

export class BridgeService {
  private readonly manager: BridgeManager;
  private state: NowPlayingState = UNAVAILABLE_STATE;
  private receivedAtMs = Date.now();
  private socket: WebSocket | undefined;
  private reconnectAttempt = 0;
  private reconnectTimer: NodeJS.Timeout | undefined;
  private disposed = false;
  private readonly artworkCache = new Map<string, string>();
  private readonly listeners = new Set<(snapshot: BridgeSnapshot) => void>();

  constructor(pluginRoot: string, tokenProvider: () => Promise<string>) {
    this.manager = new BridgeManager(pluginRoot, tokenProvider);
    this.manager.onStatusChange((status) => {
      if (status === "ready") this.openSocket();
      if (status !== "ready") this.state = UNAVAILABLE_STATE;
      this.notify();
    });
  }

  subscribe(listener: (snapshot: BridgeSnapshot) => void): () => void {
    this.listeners.add(listener);
    listener(this.snapshot());
    return () => this.listeners.delete(listener);
  }

  snapshot(): BridgeSnapshot {
    return {
      status: this.manager.getStatus(),
      state: this.state,
      receivedAtMs: this.receivedAtMs,
      info: this.manager.getInfo(),
      lastError: this.manager.getLastError(),
    };
  }

  async ensureRunning(): Promise<void> {
    try {
      await this.manager.ensureRunning();
    } catch (error) {
      streamDeck.logger.error("bridge indisponible", error);
      this.notify();
    }
  }

  /** Les réglages globaux ne sont lisibles qu'après la connexion à Stream Deck. */
  async globalSettings(): Promise<GlobalSettings | undefined> {
    try {
      return await streamDeck.settings.getGlobalSettings<GlobalSettings>();
    } catch {
      return undefined;
    }
  }

  async restart(): Promise<void> {
    this.closeSocket();
    await this.manager.reconnect().catch((error) => {
      streamDeck.logger.error("redemarrage du bridge en echec", error);
    });
    this.notify();
  }

  async dispose(): Promise<void> {
    this.disposed = true;
    this.closeSocket();
    await this.manager.stop();
  }

  /** Retourne `undefined` en cas de succès, sinon un message affichable. */
  async command(name: CommandName): Promise<string | undefined> {
    const capability = capabilityFor(name);
    if (capability && !this.state.capabilities[capability]) {
      return t("error.unsupported");
    }
    return this.post(name);
  }

  async setVolume(value: number): Promise<string | undefined> {
    if (!this.state.capabilities.volume) return t("error.unsupported");
    return this.post("volume", { value: Math.round(Math.min(100, Math.max(0, value))) });
  }

  private async post(
    endpoint: string,
    body?: Record<string, number>,
  ): Promise<string | undefined> {
    const info = this.manager.getInfo();
    if (!info) {
      await this.ensureRunning();
      return t("error.bridgeStarting");
    }

    if (!this.state.available) {
      return t("error.deezerMissing");
    }

    try {
      const response = await fetch(`http://127.0.0.1:${info.port}/v1/controls/${endpoint}`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${info.token}`,
          ...(body ? { "Content-Type": "application/json" } : {}),
        },
        body: body ? JSON.stringify(body) : undefined,
        signal: AbortSignal.timeout(5_000),
      });

      if (!response.ok) {
        const payload = (await response.json().catch(() => undefined)) as
          | { error?: { code?: string } }
          | undefined;
        return payload?.error?.code === "UNSUPPORTED_CAPABILITY"
          ? t("error.unsupported")
          : t("error.refused");
      }

      const payload = (await response.json()) as { state?: NowPlayingState };
      if (payload.state) this.applyState(payload.state);
      return undefined;
    } catch (error) {
      streamDeck.logger.warn(`commande ${endpoint} en echec`, error);
      return t("error.unreachable");
    }
  }

  private applyState(state: NowPlayingState): void {
    if (!isNewerState(this.state, state)) return;
    this.state = state;
    this.receivedAtMs = Date.now();
    this.notify();
  }

  /**
   * Pochette de la piste courante, en data URL prête pour `setImage`.
   *
   * L'URL du contrat porte une clé dérivée du contenu : elle sert directement de clé de
   * cache, sans risque de servir une image périmée.
   */
  async artworkDataUrl(): Promise<string | undefined> {
    const info = this.manager.getInfo();
    const path = this.state.artworkUrl;
    if (!info || !path) return undefined;

    const cached = this.artworkCache.get(path);
    if (cached) return cached;

    try {
      const response = await fetch(`http://127.0.0.1:${info.port}${path}`, {
        headers: { Authorization: `Bearer ${info.token}` },
        signal: AbortSignal.timeout(3_000),
      });
      if (!response.ok) return undefined;

      const buffer = Buffer.from(await response.arrayBuffer());
      if (buffer.length === 0 || buffer.length > MAX_ARTWORK_BYTES) return undefined;

      const mime = response.headers.get("content-type") ?? "image/jpeg";
      const dataUrl = `data:${mime};base64,${buffer.toString("base64")}`;

      if (this.artworkCache.size >= ARTWORK_CACHE_SIZE) {
        const oldest = this.artworkCache.keys().next().value;
        if (oldest !== undefined) this.artworkCache.delete(oldest);
      }
      this.artworkCache.set(path, dataUrl);
      return dataUrl;
    } catch {
      return undefined;
    }
  }

  private notify(): void {
    const snapshot = this.snapshot();
    for (const listener of this.listeners) listener(snapshot);
  }

  private openSocket(): void {
    if (this.disposed) return;
    const info = this.manager.getInfo();
    if (!info) return;

    this.closeSocket();

    const socket = new WebSocket(
      `ws://127.0.0.1:${info.port}/v1/events?token=${encodeURIComponent(info.token)}`,
    );
    this.socket = socket;

    socket.on("open", () => {
      this.reconnectAttempt = 0;
    });

    socket.on("message", (data) => {
      let parsed: { type?: string; payload?: unknown };
      try {
        parsed = JSON.parse(data.toString()) as { type?: string; payload?: unknown };
      } catch {
        return;
      }
      if (parsed.type === "playback.state") {
        this.applyState(parsed.payload as NowPlayingState);
      }
    });

    socket.on("close", () => {
      if (this.socket === socket) this.socket = undefined;
      this.scheduleReconnect();
    });

    socket.on("error", (error) => {
      streamDeck.logger.debug(`websocket bridge : ${error.message}`);
    });
  }

  private scheduleReconnect(): void {
    if (this.disposed) return;
    const delay =
      WS_RECONNECT_DELAYS_MS[Math.min(this.reconnectAttempt, WS_RECONNECT_DELAYS_MS.length - 1)] ??
      10_000;
    this.reconnectAttempt += 1;
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = setTimeout(() => this.openSocket(), delay);
  }

  private closeSocket(): void {
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = undefined;
    const socket = this.socket;
    this.socket = undefined;
    socket?.removeAllListeners();
    socket?.close();
  }
}

function capabilityFor(name: CommandName): keyof NowPlayingState["capabilities"] | undefined {
  switch (name) {
    case "play-pause":
      return "playPause";
    case "next":
      return "next";
    case "previous":
      return "previous";
    case "stop":
      return "stop";
    default:
      return undefined;
  }
}
