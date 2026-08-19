/**
 * Contrat de données partagé entre le bridge (Rust), le plugin Stream Deck et l'overlay.
 *
 * Toute évolution incompatible impose d'incrémenter `SCHEMA_VERSION` et de mettre à jour
 * `apps/bridge/src/contract.rs`, qui en est le miroir côté Rust.
 */

export const SCHEMA_VERSION = 1 as const;

/** Version du contrat annoncée par `/health`. Sert au plugin à refuser un bridge incompatible. */
export const CONTRACT_VERSION = "1.0.0";

export type PlaybackStatus = "playing" | "paused" | "stopped" | "unavailable";

export type PlaybackSource = "deezer-desktop" | "deezer-inapp" | "unknown";

/**
 * Capacités *effectives*. Le spike M0 a montré que Deezer déclare `IsPreviousEnabled=false`
 * en permanence alors que la commande fonctionne : le bridge combine donc les capacités
 * déclarées par l'OS et celles observées à l'usage.
 *
 * Déclaré en alias de type (et non en interface) pour rester assignable aux types JSON
 * exigés par le SDK Stream Deck.
 */
export type PlaybackCapabilities = {
  playPause: boolean;
  next: boolean;
  previous: boolean;
  stop: boolean;
  volume: boolean;
  seek: boolean;
  shuffle: boolean;
  repeat: boolean;
};

export type NowPlayingState = {
  schemaVersion: typeof SCHEMA_VERSION;
  source: PlaybackSource;
  sourceLabel?: string;
  available: boolean;
  status: PlaybackStatus;

  trackId?: string;
  title?: string;
  artist?: string;
  album?: string;
  artworkUrl?: string;
  artworkDataUrl?: string;

  positionMs?: number;
  durationMs?: number;
  volume?: number;

  capabilities: PlaybackCapabilities;
  /** ISO 8601 UTC. */
  updatedAt: string;
  /** Strictement croissant. Permet d'ignorer un événement arrivé dans le désordre. */
  sequence: number;
};

export type BridgeErrorCode =
  | "PLAYER_NOT_FOUND"
  | "UNSUPPORTED_CAPABILITY"
  | "COMMAND_FAILED"
  | "TOKEN_INVALID"
  | "BRIDGE_START_FAILED"
  | "INTERNAL_ERROR";

export type BridgeError = {
  code: BridgeErrorCode;
  message: string;
  retryable: boolean;
  timestamp: string;
};

export type HealthResponse = {
  ready: boolean;
  version: string;
  contractVersion: string;
  schemaVersion: typeof SCHEMA_VERSION;
  platform: string;
  arch: string;
  adapter: string;
  uptimeMs: number;
};

export type BridgeEvent =
  | { type: "bridge.ready"; payload: { version: string } }
  | { type: "playback.state"; payload: NowPlayingState }
  | { type: "playback.error"; payload: BridgeError }
  | { type: "bridge.shutdown"; payload: { reason: string } };

export type OkResponse<T = unknown> = {
  ok: true;
  state?: NowPlayingState;
  data?: T;
};

export type ErrResponse = {
  ok: false;
  error: BridgeError;
};

export const NO_CAPABILITIES: PlaybackCapabilities = Object.freeze({
  playPause: false,
  next: false,
  previous: false,
  stop: false,
  volume: false,
  seek: false,
  shuffle: false,
  repeat: false,
});

export const UNAVAILABLE_STATE: NowPlayingState = Object.freeze({
  schemaVersion: SCHEMA_VERSION,
  source: "unknown",
  available: false,
  status: "unavailable",
  capabilities: NO_CAPABILITIES,
  updatedAt: new Date(0).toISOString(),
  sequence: 0,
});

/**
 * Position à afficher, extrapolée localement pour rester fluide sans flux d'événements
 * à haute fréquence. `receivedAtMs` est l'horodatage local de réception de l'état.
 */
export function effectivePositionMs(
  state: NowPlayingState,
  receivedAtMs: number,
  nowMs: number = Date.now(),
): number | undefined {
  if (state.positionMs === undefined) return undefined;
  if (state.status !== "playing") return state.positionMs;

  const elapsed = Math.max(0, nowMs - receivedAtMs);
  const projected = state.positionMs + elapsed;
  if (state.durationMs === undefined) return projected;
  return Math.min(projected, state.durationMs);
}

/** `true` si `candidate` doit remplacer `current` (§10.2 : on ignore les événements périmés). */
export function isNewerState(current: NowPlayingState | undefined, candidate: NowPlayingState): boolean {
  if (!current) return true;
  return candidate.sequence >= current.sequence;
}

export function formatDuration(ms: number | undefined): string {
  if (ms === undefined || !Number.isFinite(ms) || ms < 0) return "--:--";
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}
