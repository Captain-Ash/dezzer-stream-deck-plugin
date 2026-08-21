/**
 * Génère les PNG du plugin.
 *
 * L'icône du plugin et celle de la catégorie proviennent de `assets/deezer-logo.png`.
 * Les icônes d'action sont dessinées par un rasteriseur minimal : leurs glyphes ne sont
 * que des rectangles, cercles et triangles.
 */

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { encodePng, readPng, resize } from "./lib/png.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const imgsDir = join(root, "apps/streamdeck-plugin/com.dezzer.deezer.sdPlugin/imgs");
const logoPath = join(root, "assets/deezer-logo.png");

const PALETTE = {
  background: [16, 16, 24, 255],
  halo: [162, 56, 255, 70],
  glyph: [255, 255, 255, 255],
};

class Canvas {
  constructor(size) {
    this.size = size;
    this.pixels = new Uint8ClampedArray(size * size * 4);
  }

  blend(x, y, [r, g, b, a], coverage = 1) {
    if (x < 0 || y < 0 || x >= this.size || y >= this.size) return;
    const alpha = (a / 255) * coverage;
    if (alpha <= 0) return;

    const index = (y * this.size + x) * 4;
    const dstA = this.pixels[index + 3] / 255;
    const outA = alpha + dstA * (1 - alpha);
    if (outA <= 0) return;

    for (let channel = 0; channel < 3; channel += 1) {
      const src = [r, g, b][channel];
      const dst = this.pixels[index + channel];
      this.pixels[index + channel] = (src * alpha + dst * dstA * (1 - alpha)) / outA;
    }
    this.pixels[index + 3] = outA * 255;
  }

  /** Anticrénelage par supersampling 4x4 d'une fonction d'appartenance. */
  fill(colour, inside) {
    const steps = 4;
    for (let y = 0; y < this.size; y += 1) {
      for (let x = 0; x < this.size; x += 1) {
        let hits = 0;
        for (let sy = 0; sy < steps; sy += 1) {
          for (let sx = 0; sx < steps; sx += 1) {
            if (inside(x + (sx + 0.5) / steps, y + (sy + 0.5) / steps)) hits += 1;
          }
        }
        if (hits > 0) this.blend(x, y, colour, hits / (steps * steps));
      }
    }
  }

  roundedRect(x0, y0, x1, y1, radius, colour) {
    this.fill(colour, (px, py) => {
      if (px < x0 || px > x1 || py < y0 || py > y1) return false;
      const cx = Math.min(Math.max(px, x0 + radius), x1 - radius);
      const cy = Math.min(Math.max(py, y0 + radius), y1 - radius);
      return (px - cx) ** 2 + (py - cy) ** 2 <= radius ** 2;
    });
  }

  circle(cx, cy, radius, colour) {
    this.fill(colour, (px, py) => (px - cx) ** 2 + (py - cy) ** 2 <= radius ** 2);
  }

  ring(cx, cy, radius, thickness, colour) {
    const inner = radius - thickness;
    this.fill(colour, (px, py) => {
      const distance = Math.hypot(px - cx, py - cy);
      return distance <= radius && distance >= inner;
    });
  }

  triangle(points, colour) {
    const [[ax, ay], [bx, by], [cx, cy]] = points;
    const sign = (px, py, qx, qy, rx, ry) => (px - rx) * (qy - ry) - (qx - rx) * (py - ry);
    this.fill(colour, (px, py) => {
      const d1 = sign(px, py, ax, ay, bx, by);
      const d2 = sign(px, py, bx, by, cx, cy);
      const d3 = sign(px, py, cx, cy, ax, ay);
      return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
    });
  }

  /** Compose une image RGBA centrée, avec une marge exprimée en fraction de la taille. */
  drawImage(image, margin = 0) {
    const inset = Math.round(this.size * margin);
    const target = this.size - inset * 2;
    if (target <= 0) return;

    const scaled = resize(image, target);
    for (let y = 0; y < target; y += 1) {
      for (let x = 0; x < target; x += 1) {
        const at = (y * target + x) * 4;
        this.blend(x + inset, y + inset, [
          scaled.pixels[at],
          scaled.pixels[at + 1],
          scaled.pixels[at + 2],
          scaled.pixels[at + 3],
        ]);
      }
    }
  }

  toPng() {
    return encodePng(this.size, this.pixels);
  }
}

const GLYPHS = {
  play: (c, u) => c.triangle([[0.38, 0.28], [0.72, 0.5], [0.38, 0.72]].map(u), PALETTE.glyph),
  pause: (c, u) => {
    const [x1, y1] = u([0.38, 0.29]);
    const [x2, y2] = u([0.47, 0.71]);
    c.roundedRect(x1, y1, x2, y2, c.size * 0.02, PALETTE.glyph);
    const [x3, y3] = u([0.53, 0.29]);
    const [x4, y4] = u([0.62, 0.71]);
    c.roundedRect(x3, y3, x4, y4, c.size * 0.02, PALETTE.glyph);
  },
  next: (c, u) => {
    c.triangle([[0.32, 0.29], [0.6, 0.5], [0.32, 0.71]].map(u), PALETTE.glyph);
    const [x1, y1] = u([0.63, 0.29]);
    const [x2, y2] = u([0.7, 0.71]);
    c.roundedRect(x1, y1, x2, y2, c.size * 0.015, PALETTE.glyph);
  },
  previous: (c, u) => {
    c.triangle([[0.68, 0.29], [0.4, 0.5], [0.68, 0.71]].map(u), PALETTE.glyph);
    const [x1, y1] = u([0.3, 0.29]);
    const [x2, y2] = u([0.37, 0.71]);
    c.roundedRect(x1, y1, x2, y2, c.size * 0.015, PALETTE.glyph);
  },
  "now-playing": (c, u) => {
    for (const [x, height] of [
      [0.3, 0.18],
      [0.42, 0.32],
      [0.54, 0.25],
      [0.66, 0.12],
    ]) {
      const [x1, y1] = u([x, 0.7 - height]);
      const [x2, y2] = u([x + 0.08, 0.7]);
      c.roundedRect(x1, y1, x2, y2, c.size * 0.015, PALETTE.glyph);
    }
  },
  volume: (c, u) => {
    c.triangle([[0.28, 0.42], [0.4, 0.42], [0.4, 0.58]].map(u), PALETTE.glyph);
    c.triangle([[0.4, 0.42], [0.54, 0.28], [0.4, 0.58]].map(u), PALETTE.glyph);
    c.triangle([[0.54, 0.28], [0.54, 0.72], [0.4, 0.58]].map(u), PALETTE.glyph);
    const [cx, cy] = u([0.56, 0.5]);
    c.ring(cx, cy, c.size * 0.17, c.size * 0.045, PALETTE.glyph);
  },
  diagnostics: (c, u) => {
    const [cx, cy] = u([0.5, 0.5]);
    c.ring(cx, cy, c.size * 0.22, c.size * 0.055, PALETTE.glyph);
    const [x1, y1] = u([0.47, 0.35]);
    const [x2, y2] = u([0.53, 0.53]);
    c.roundedRect(x1, y1, x2, y2, c.size * 0.02, PALETTE.glyph);
    const [dx, dy] = u([0.5, 0.62]);
    c.circle(dx, dy, c.size * 0.035, PALETTE.glyph);
  },
};

function drawGlyph(size, glyph, { background = true } = {}) {
  const canvas = new Canvas(size);
  const unit = ([x, y]) => [x * size, y * size];
  if (background) {
    canvas.roundedRect(0, 0, size, size, size * 0.18, PALETTE.background);
    canvas.circle(size * 0.5, size * 0.46, size * 0.32, PALETTE.halo);
  }
  GLYPHS[glyph](canvas, unit);
  return canvas.toPng();
}

function drawLogo(logo, size, { background = true, margin = 0.1 } = {}) {
  const canvas = new Canvas(size);
  if (background) {
    canvas.roundedRect(0, 0, size, size, size * 0.18, PALETTE.background);
  }
  canvas.drawImage(logo, margin);
  return canvas.toPng();
}

async function emit(relativePath, size, render) {
  const target = join(imgsDir, relativePath);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(`${target}.png`, render(size));
  await writeFile(`${target}@2x.png`, render(size * 2));
  console.log(`  ${relativePath} (${size}px)`);
}

const ACTIONS = {
  "play-pause": "play",
  next: "next",
  previous: "previous",
  "now-playing": "now-playing",
  "volume-up": "volume",
  "volume-down": "volume",
  diagnostics: "diagnostics",
};

const logo = await readPng(logoPath);
console.log(`logo source : ${logo.width}x${logo.height}`);

// Icone de boutique : fond opaque, comme l'exige Elgato.
await emit("plugin/marketplace", 256, (size) => drawLogo(logo, size));
// Icone de categorie : transparente, posee sur le fond de l'application.
await emit("plugin/category-icon", 28, (size) =>
  drawLogo(logo, size, { background: false, margin: 0 }),
);

for (const [name, glyph] of Object.entries(ACTIONS)) {
  await emit(`actions/${name}/icon`, 20, (size) => drawGlyph(size, glyph, { background: false }));
  await emit(`actions/${name}/key`, 72, (size) => drawGlyph(size, glyph));
}

console.log("termine");
