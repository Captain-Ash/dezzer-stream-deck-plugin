/**
 * Connexion au bridge : état initial en HTTP puis flux WebSocket, avec reconnexion
 * automatique à intervalles croissants. L'overlay doit survivre à un redémarrage du
 * bridge ou de Stream Deck sans intervention (§14.2).
 */

import type { BridgeEvent, NowPlayingState } from "@dezzer/playback-contract";

export interface ConnectionHandlers {
  onState: (state: NowPlayingState, receivedAtMs: number) => void;
  onConnectionChange: (connected: boolean) => void;
  /** Fourni uniquement quand le visualiseur est demandé : sinon le flux reste fermé. */
  onSpectrum?: (bands: number[]) => void;
}

const RECONNECT_DELAYS_MS = [500, 1_000, 2_000, 5_000, 10_000];

export class BridgeConnection {
  private socket: WebSocket | undefined;
  private attempt = 0;
  private closed = false;
  private reconnectTimer: number | undefined;
  private levelSocket: WebSocket | undefined;
  private levelAttempt = 0;
  private levelTimer: number | undefined;

  constructor(
    private readonly token: string,
    private readonly handlers: ConnectionHandlers,
  ) {}

  start(): void {
    void this.loadInitialState();
    this.openSocket();
    if (this.handlers.onSpectrum) this.openLevelSocket();
  }

  stop(): void {
    this.closed = true;
    if (this.reconnectTimer !== undefined) {
      window.clearTimeout(this.reconnectTimer);
    }
    if (this.levelTimer !== undefined) {
      window.clearTimeout(this.levelTimer);
    }
    this.socket?.close();
    this.levelSocket?.close();
  }

  private async loadInitialState(): Promise<void> {
    try {
      const response = await fetch("/v1/state", {
        headers: { Authorization: `Bearer ${this.token}` },
        cache: "no-store",
      });
      if (!response.ok) return;
      const body = (await response.json()) as { ok: boolean; state?: NowPlayingState };
      if (body.state) {
        this.handlers.onState(body.state, Date.now());
      }
    } catch {
      // Le WebSocket prendra le relais : rien à signaler à l'écran.
    }
  }

  private openSocket(): void {
    if (this.closed) return;

    const url = this.socketUrl("/v1/events");

    let socket: WebSocket;
    try {
      socket = new WebSocket(url);
    } catch {
      this.scheduleReconnect();
      return;
    }

    this.socket = socket;

    socket.addEventListener("open", () => {
      this.attempt = 0;
      this.handlers.onConnectionChange(true);
    });

    socket.addEventListener("message", (event) => {
      if (typeof event.data !== "string") return;
      let parsed: BridgeEvent;
      try {
        parsed = JSON.parse(event.data) as BridgeEvent;
      } catch {
        return;
      }
      if (parsed.type === "playback.state") {
        this.handlers.onState(parsed.payload, Date.now());
      }
      // Les erreurs du bridge ne sont jamais affichées dans le flux (§10.1).
    });

    socket.addEventListener("close", () => {
      this.handlers.onConnectionChange(false);
      this.scheduleReconnect();
    });

    socket.addEventListener("error", () => {
      socket.close();
    });
  }

  private scheduleReconnect(): void {
    if (this.closed) return;
    const delay =
      RECONNECT_DELAYS_MS[Math.min(this.attempt, RECONNECT_DELAYS_MS.length - 1)] ?? 10_000;
    this.attempt += 1;
    this.reconnectTimer = window.setTimeout(() => this.openSocket(), delay);
  }

  private openLevelSocket(): void {
    if (this.closed) return;

    const url = this.socketUrl("/v1/levels");

    let socket: WebSocket;
    try {
      socket = new WebSocket(url);
    } catch {
      this.scheduleLevelReconnect();
      return;
    }

    this.levelSocket = socket;

    socket.addEventListener("open", () => {
      this.levelAttempt = 0;
    });

    socket.addEventListener("message", (event) => {
      if (typeof event.data !== "string") return;
      let parsed: { type?: string; payload?: { bands?: unknown } };
      try {
        parsed = JSON.parse(event.data) as { type?: string; payload?: { bands?: unknown } };
      } catch {
        return;
      }
      const bands = parsed.payload?.bands;
      if (parsed.type === "playback.spectrum" && Array.isArray(bands)) {
        this.handlers.onSpectrum?.(bands as number[]);
      }
    });

    socket.addEventListener("close", () => this.scheduleLevelReconnect());
    socket.addEventListener("error", () => socket.close());
  }

  private scheduleLevelReconnect(): void {
    if (this.closed) return;
    const delay =
      RECONNECT_DELAYS_MS[Math.min(this.levelAttempt, RECONNECT_DELAYS_MS.length - 1)] ?? 10_000;
    this.levelAttempt += 1;
    this.levelTimer = window.setTimeout(() => this.openLevelSocket(), delay);
  }

  private socketUrl(path: string): string {
    const url = new URL(path, window.location.href);
    url.protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    url.searchParams.set("token", this.token);
    return url.toString();
  }
}
