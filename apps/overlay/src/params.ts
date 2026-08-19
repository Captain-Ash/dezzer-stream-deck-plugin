/**
 * Lecture et validation des paramètres d'URL.
 *
 * Toute valeur venant de la query string est traitée comme hostile : aucune n'est
 * insérée telle quelle dans le DOM ou dans une feuille de style (§10.3).
 */

export const THEMES = ["minimal", "glass", "neon", "broadcast"] as const;
export type Theme = (typeof THEMES)[number];

export interface OverlayOptions {
  token: string;
  theme: Theme;
  width: number;
  showAlbum: boolean;
  showTime: boolean;
  showArtwork: boolean;
  waveform: boolean;
  autoHide: boolean;
  hideAfterMs: number;
  accent?: string;
}

const DEFAULTS = {
  theme: "glass" as Theme,
  width: 720,
  showAlbum: false,
  showTime: true,
  showArtwork: true,
  waveform: false,
  autoHide: false,
  hideAfterMs: 10_000,
};

const WIDTH_RANGE = { min: 400, max: 1200 };
const HIDE_AFTER_RANGE = { min: 1_000, max: 600_000 };

/** Trois, quatre, six ou huit chiffres hexadécimaux, précédés d'un `#`. */
const HEX_COLOUR = /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

export function parseOptions(search: string): OverlayOptions {
  const params = new URLSearchParams(search);

  return {
    token: params.get("token") ?? "",
    theme: parseTheme(params.get("theme")),
    width: parseBoundedInt(params.get("width"), DEFAULTS.width, WIDTH_RANGE),
    showAlbum: parseBool(params.get("showAlbum"), DEFAULTS.showAlbum),
    showTime: parseBool(params.get("showTime"), DEFAULTS.showTime),
    showArtwork: parseBool(params.get("showArtwork"), DEFAULTS.showArtwork),
    waveform: parseBool(params.get("waveform"), DEFAULTS.waveform),
    autoHide: parseBool(params.get("autoHide"), DEFAULTS.autoHide),
    hideAfterMs: parseBoundedInt(params.get("hideAfterMs"), DEFAULTS.hideAfterMs, HIDE_AFTER_RANGE),
    accent: parseAccent(params.get("accent")),
  };
}

export function parseTheme(raw: string | null): Theme {
  const value = raw?.trim().toLowerCase();
  return THEMES.includes(value as Theme) ? (value as Theme) : DEFAULTS.theme;
}

export function parseBool(raw: string | null, fallback: boolean): boolean {
  if (raw === "1" || raw === "true") return true;
  if (raw === "0" || raw === "false") return false;
  return fallback;
}

export function parseBoundedInt(
  raw: string | null,
  fallback: number,
  range: { min: number; max: number },
): number {
  if (raw === null) return fallback;
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value)) return fallback;
  return Math.min(range.max, Math.max(range.min, value));
}

/** Une couleur non conforme est ignorée : jamais injectée dans le CSS. */
export function parseAccent(raw: string | null): string | undefined {
  if (raw === null) return undefined;
  const value = raw.trim();
  return HEX_COLOUR.test(value) ? value : undefined;
}
