/**
 * Composition des images de touche, en SVG encodé en data URL.
 *
 * Stream Deck ne sait pas faire défiler un titre : `setTitle` est statique et tronque.
 * On dessine donc le texte dans l'image, ce qui permet à la fois d'y poser la pochette et
 * d'animer un défilement en réémettant l'image.
 */

const SIZE = 144;

export type KeyMood = "active" | "idle" | "disabled" | "error";

const HALO: Record<KeyMood, string> = {
  active: "#a238ff",
  idle: "#3a3a4a",
  disabled: "#23232c",
  error: "#c0392b",
};

const GLYPH: Record<KeyMood, string> = {
  active: "#ffffff",
  idle: "#e8e8ef",
  disabled: "#6b6b78",
  error: "#ffffff",
};

export interface Artwork {
  /** Data URL complète, `data:image/jpeg;base64,…`. */
  dataUrl: string;
}

interface Layer {
  artwork?: Artwork;
  /** Assombrissement de la pochette, de 0 à 1. */
  scrim?: number;
  mood: KeyMood;
}

function background({ artwork, scrim = 0, mood }: Layer): string {
  if (!artwork) {
    return `<rect width="${SIZE}" height="${SIZE}" rx="26" fill="#101018"/>
<circle cx="72" cy="66" r="46" fill="${HALO[mood]}" fill-opacity="0.22"/>`;
  }

  // `href` et `xlink:href` : les moteurs SVG embarqués ne gèrent pas tous les deux formes.
  return `<defs><clipPath id="k"><rect width="${SIZE}" height="${SIZE}" rx="26"/></clipPath></defs>
<g clip-path="url(#k)">
<image href="${artwork.dataUrl}" xlink:href="${artwork.dataUrl}" x="0" y="0" width="${SIZE}" height="${SIZE}" preserveAspectRatio="xMidYMid slice"/>
<rect width="${SIZE}" height="${SIZE}" fill="#000000" fill-opacity="${scrim.toFixed(2)}"/>
</g>`;
}

function svg(body: string): string {
  const document = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}">${body}</svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(document, "utf8").toString("base64")}`;
}

export function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

/**
 * Largeur approchée d'une chaîne. Un moteur SVG n'expose aucune mesure ; le facteur 0,54
 * correspond à la largeur moyenne d'un caractère de Segoe UI et suffit à décider s'il faut
 * faire défiler.
 */
export function estimateTextWidth(text: string, fontSize: number, bold = false): number {
  return text.length * fontSize * (bold ? 0.58 : 0.54);
}

const SCROLL_SPEED_PX_PER_S = 26;
const SCROLL_PAUSE_MS = 1_200;

/**
 * Décalage horizontal d'un texte défilant, en aller-retour avec une pause à chaque bout.
 * La fonction est pure : l'animation ne dépend que du temps écoulé.
 */
export function scrollOffset(overflow: number, elapsedMs: number): number {
  if (overflow <= 0) return 0;

  const travelMs = (overflow / SCROLL_SPEED_PX_PER_S) * 1_000;
  const cycleMs = travelMs * 2 + SCROLL_PAUSE_MS * 2;
  const t = elapsedMs % cycleMs;

  if (t < SCROLL_PAUSE_MS) return 0;
  if (t < SCROLL_PAUSE_MS + travelMs) {
    return -overflow * ((t - SCROLL_PAUSE_MS) / travelMs);
  }
  if (t < SCROLL_PAUSE_MS * 2 + travelMs) return -overflow;
  return -overflow * (1 - (t - SCROLL_PAUSE_MS * 2 - travelMs) / travelMs);
}

interface ScrollingLine {
  text: string;
  y: number;
  fontSize: number;
  bold?: boolean;
  opacity?: number;
}

const TEXT_MARGIN = 8;
const TEXT_VIEWPORT = SIZE - TEXT_MARGIN * 2;

function line(entry: ScrollingLine, elapsedMs: number, index: number): string {
  const { text, y, fontSize, bold = false, opacity = 1 } = entry;
  if (!text) return "";

  const width = estimateTextWidth(text, fontSize, bold);
  const escaped = escapeXml(text);
  const weight = bold ? 650 : 400;
  const style =
    `font-family="Segoe UI, Tahoma, Arial, sans-serif" font-size="${fontSize}" ` +
    `font-weight="${weight}" fill="#ffffff" fill-opacity="${opacity}"`;

  if (width <= TEXT_VIEWPORT) {
    return `<text x="${SIZE / 2}" y="${y}" text-anchor="middle" ${style}>${escaped}</text>`;
  }

  const offset = scrollOffset(width - TEXT_VIEWPORT, elapsedMs);
  return `<defs><clipPath id="c${index}"><rect x="${TEXT_MARGIN}" y="${y - fontSize}" width="${TEXT_VIEWPORT}" height="${fontSize + 6}"/></clipPath></defs>
<g clip-path="url(#c${index})"><text x="${TEXT_MARGIN + offset}" y="${y}" ${style}>${escaped}</text></g>`;
}

export interface NowPlayingKey {
  title: string;
  artist: string;
  time?: string;
  playing: boolean;
  artwork?: Artwork;
  elapsedMs: number;
}

export function nowPlayingKey(key: NowPlayingKey): string {
  const scrim = key.artwork ? 0.42 : 0;
  const lines: ScrollingLine[] = [
    { text: key.title, y: 96, fontSize: 19, bold: true },
    { text: key.artist, y: 118, fontSize: 15, opacity: 0.8 },
  ];

  const gradient = key.artwork
    ? `<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1">
<stop offset="0%" stop-color="#000000" stop-opacity="0"/>
<stop offset="100%" stop-color="#000000" stop-opacity="0.85"/>
</linearGradient></defs>
<rect x="0" y="58" width="${SIZE}" height="${SIZE - 58}" fill="url(#s)"/>`
    : "";

  const time = key.time
    ? `<text x="${SIZE / 2}" y="138" text-anchor="middle" font-family="Segoe UI, Tahoma, Arial, sans-serif" font-size="14" fill="#ffffff" fill-opacity="0.75">${
        key.playing ? "\u25b6" : "\u23f8"
      } ${escapeXml(key.time)}</text>`
    : "";

  return svg(
    background({ artwork: key.artwork, scrim, mood: key.playing ? "active" : "idle" }) +
      gradient +
      lines.map((entry, index) => line(entry, key.elapsedMs, index)).join("") +
      time,
  );
}

export interface GlyphKey {
  glyph: "play" | "pause" | "next" | "previous" | "volume-up" | "volume-down" | "diagnostics";
  mood: KeyMood;
  artwork?: Artwork;
}

export function glyphKey({ glyph, mood, artwork }: GlyphKey): string {
  const colour = GLYPH[mood];
  // Sur une pochette, le glyphe doit rester lisible quelle que soit l'image.
  const scrim = artwork ? 0.5 : 0;
  return svg(background({ artwork, scrim, mood }) + GLYPHS[glyph](colour));
}

const GLYPHS: Record<GlyphKey["glyph"], (colour: string) => string> = {
  play: (c) => `<path d="M58 44 L104 72 L58 100 Z" fill="${c}"/>`,
  pause: (c) =>
    `<rect x="55" y="46" width="14" height="52" rx="4" fill="${c}"/>` +
    `<rect x="79" y="46" width="14" height="52" rx="4" fill="${c}"/>`,
  next: (c) =>
    `<path d="M48 48 L84 72 L48 96 Z" fill="${c}"/><rect x="89" y="48" width="10" height="48" rx="3" fill="${c}"/>`,
  previous: (c) =>
    `<path d="M96 48 L60 72 L96 96 Z" fill="${c}"/><rect x="45" y="48" width="10" height="48" rx="3" fill="${c}"/>`,
  "volume-up": (c) =>
    `<path d="M42 62 L56 62 L74 46 L74 98 L56 82 L42 82 Z" fill="${c}"/>` +
    `<path d="M86 58 A22 22 0 0 1 86 86" fill="none" stroke="${c}" stroke-width="7" stroke-linecap="round"/>` +
    `<path d="M96 48 A34 34 0 0 1 96 96" fill="none" stroke="${c}" stroke-width="7" stroke-linecap="round"/>`,
  "volume-down": (c) =>
    `<path d="M48 62 L62 62 L80 46 L80 98 L62 82 L48 82 Z" fill="${c}"/>` +
    `<path d="M92 58 A22 22 0 0 1 92 86" fill="none" stroke="${c}" stroke-width="7" stroke-linecap="round"/>`,
  diagnostics: (c) =>
    `<circle cx="72" cy="72" r="30" fill="none" stroke="${c}" stroke-width="8"/>` +
    `<rect x="67" y="54" width="10" height="24" rx="5" fill="${c}"/><circle cx="72" cy="88" r="6" fill="${c}"/>`,
};

/** Touche de repli quand rien n'est disponible : glyphe grisé et libellé court. */
export function unavailableKey(glyph: GlyphKey["glyph"], label: string): string {
  // SVG ignore les sauts de ligne : chaque ligne est un `<text>` distinct.
  const lines = label.split("\n").filter(Boolean);
  const firstBaseline = lines.length > 1 ? 120 : 132;

  const text = lines
    .map(
      (value, index) =>
        `<text x="${SIZE / 2}" y="${firstBaseline + index * 17}" text-anchor="middle" font-family="Segoe UI, Tahoma, Arial, sans-serif" font-size="15" fill="#8a8a99">${escapeXml(value)}</text>`,
    )
    .join("");

  return svg(background({ mood: "disabled" }) + GLYPHS[glyph](GLYPH.disabled) + text);
}
