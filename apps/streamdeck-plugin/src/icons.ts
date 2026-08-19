/**
 * Icônes de touche générées en SVG puis encodées en data URL.
 *
 * Cela évite d'embarquer des dizaines de PNG et garantit un rendu net sur toutes les
 * générations de Stream Deck.
 */

const SIZE = 144;

export type KeyMood = "active" | "idle" | "disabled" | "error";

const COLOURS: Record<KeyMood, { glyph: string; halo: string }> = {
  active: { glyph: "#ffffff", halo: "#a238ff" },
  idle: { glyph: "#e8e8ef", halo: "#3a3a4a" },
  disabled: { glyph: "#6b6b78", halo: "#23232c" },
  error: { glyph: "#ffffff", halo: "#c0392b" },
};

function wrap(mood: KeyMood, body: string): string {
  const { halo } = COLOURS[mood];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${SIZE}" height="${SIZE}" viewBox="0 0 ${SIZE} ${SIZE}">
<rect width="${SIZE}" height="${SIZE}" rx="26" fill="#101018"/>
<circle cx="72" cy="66" r="46" fill="${halo}" fill-opacity="0.22"/>
${body}
</svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(svg, "utf8").toString("base64")}`;
}

export function playIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  return wrap(mood, `<path d="M58 42 L104 66 L58 90 Z" fill="${glyph}"/>`);
}

export function pauseIcon(mood: KeyMood = "active"): string {
  const { glyph } = COLOURS[mood];
  return wrap(
    mood,
    `<rect x="56" y="42" width="13" height="48" rx="4" fill="${glyph}"/>
<rect x="79" y="42" width="13" height="48" rx="4" fill="${glyph}"/>`,
  );
}

export function nextIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  return wrap(
    mood,
    `<path d="M50 44 L84 66 L50 88 Z" fill="${glyph}"/>
<rect x="90" y="44" width="10" height="44" rx="3" fill="${glyph}"/>`,
  );
}

export function previousIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  return wrap(
    mood,
    `<path d="M94 44 L60 66 L94 88 Z" fill="${glyph}"/>
<rect x="44" y="44" width="10" height="44" rx="3" fill="${glyph}"/>`,
  );
}

export function nowPlayingIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  const bars = [
    { x: 48, h: 26 },
    { x: 63, h: 44 },
    { x: 78, h: 34 },
    { x: 93, h: 18 },
  ]
    .map(({ x, h }) => `<rect x="${x}" y="${90 - h}" width="9" height="${h}" rx="3" fill="${glyph}"/>`)
    .join("");
  return wrap(mood, bars);
}

export function diagnosticsIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  return wrap(
    mood,
    `<circle cx="72" cy="66" r="30" fill="none" stroke="${glyph}" stroke-width="8"/>
<rect x="67" y="48" width="10" height="24" rx="5" fill="${glyph}"/>
<circle cx="72" cy="82" r="6" fill="${glyph}"/>`,
  );
}

export function overlayIcon(mood: KeyMood = "idle"): string {
  const { glyph } = COLOURS[mood];
  return wrap(
    mood,
    `<rect x="40" y="44" width="64" height="44" rx="7" fill="none" stroke="${glyph}" stroke-width="7"/>
<rect x="50" y="72" width="44" height="7" rx="3.5" fill="${glyph}"/>`,
  );
}
