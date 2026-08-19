/** Réglages persistés par Stream Deck. Aucun credential Deezer n'y figure (§9.5). */

export const OVERLAY_THEMES = ["minimal", "glass", "neon", "broadcast"] as const;
export type OverlayTheme = (typeof OVERLAY_THEMES)[number];

export type NowPlayingFormat = "title" | "artist" | "title-artist";

export type OverlaySettings = {
  theme: OverlayTheme;
  width: number;
  showAlbum: boolean;
  showTime: boolean;
  showArtwork: boolean;
  waveform: boolean;
  autoHide: boolean;
  hideAfterMs: number;
  accent: string;
};

export type GlobalSettings = {
  /**
   * Jeton d'installation. Généré une seule fois puis réutilisé, afin que l'URL collée
   * dans OBS reste valable après un redémarrage.
   */
  token?: string;
  nowPlayingFormat?: NowPlayingFormat;
  /** Pochette de l'album en fond des touches Play/Pause et Morceau en cours. */
  showArtworkOnKeys?: boolean;
  /** Pas appliqué par les actions Volume + et Volume -, en pourcentage. */
  volumeStep?: number;
  overlay?: Partial<OverlaySettings>;
};

export const VOLUME_STEPS = [1, 2, 5, 10] as const;

export function normaliseVolumeStep(value: unknown): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  return (VOLUME_STEPS as readonly number[]).includes(parsed) ? parsed : 5;
}

export const DEFAULT_OVERLAY: OverlaySettings = {
  theme: "glass",
  width: 720,
  showAlbum: false,
  showTime: true,
  showArtwork: true,
  waveform: false,
  autoHide: false,
  hideAfterMs: 10_000,
  accent: "",
};

export function normaliseOverlay(settings: Partial<OverlaySettings> | undefined): OverlaySettings {
  const merged = { ...DEFAULT_OVERLAY, ...(settings ?? {}) };
  return {
    theme: OVERLAY_THEMES.includes(merged.theme) ? merged.theme : DEFAULT_OVERLAY.theme,
    width: clamp(merged.width, 400, 1200, DEFAULT_OVERLAY.width),
    showAlbum: Boolean(merged.showAlbum),
    showTime: Boolean(merged.showTime),
    showArtwork: Boolean(merged.showArtwork),
    waveform: Boolean(merged.waveform),
    autoHide: Boolean(merged.autoHide),
    hideAfterMs: clamp(merged.hideAfterMs, 1_000, 600_000, DEFAULT_OVERLAY.hideAfterMs),
    accent: /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(merged.accent ?? "")
      ? merged.accent
      : "",
  };
}

export function buildOverlayUrl(port: number, token: string, settings: OverlaySettings): string {
  const url = new URL(`http://127.0.0.1:${port}/overlay/`);
  url.searchParams.set("token", token);
  url.searchParams.set("theme", settings.theme);
  url.searchParams.set("width", String(settings.width));
  url.searchParams.set("showAlbum", settings.showAlbum ? "1" : "0");
  url.searchParams.set("showTime", settings.showTime ? "1" : "0");
  url.searchParams.set("showArtwork", settings.showArtwork ? "1" : "0");
  if (settings.waveform) {
    url.searchParams.set("waveform", "1");
  }
  if (settings.autoHide) {
    url.searchParams.set("autoHide", "1");
    url.searchParams.set("hideAfterMs", String(settings.hideAfterMs));
  }
  if (settings.accent) {
    url.searchParams.set("accent", settings.accent);
  }
  return url.toString();
}

function clamp(value: unknown, min: number, max: number, fallback: number): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
}
