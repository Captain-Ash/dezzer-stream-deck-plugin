/** Décodeur PNG minimal (8 bits, non entrelacé), redimensionnement et encodeur RGBA. */

import { readFile } from "node:fs/promises";
import { deflateSync, inflateSync } from "node:zlib";

export async function readPng(path) {
  return decodePng(await readFile(path));
}

export function decodePng(buffer) {
  if (buffer.readUInt32BE(0) !== 0x89504e47) throw new Error("signature PNG invalide");

  let offset = 8;
  let header;
  const data = [];
  let palette;
  let transparency;

  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.toString("ascii", offset + 4, offset + 8);
    const body = buffer.subarray(offset + 8, offset + 8 + length);
    offset += length + 12;

    if (type === "IHDR") {
      header = {
        width: body.readUInt32BE(0),
        height: body.readUInt32BE(4),
        depth: body[8],
        colourType: body[9],
        interlace: body[12],
      };
    } else if (type === "PLTE") {
      palette = body;
    } else if (type === "tRNS") {
      transparency = body;
    } else if (type === "IDAT") {
      data.push(body);
    } else if (type === "IEND") {
      break;
    }
  }

  if (!header) throw new Error("chunk IHDR absent");
  if (header.depth !== 8) throw new Error(`profondeur ${header.depth} non gérée`);
  if (header.interlace !== 0) throw new Error("PNG entrelacé non géré");

  const channels = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[header.colourType];
  if (!channels) throw new Error(`type de couleur ${header.colourType} non géré`);

  const raw = inflateSync(Buffer.concat(data));
  const { width, height } = header;
  const stride = width * channels;
  const pixels = new Uint8ClampedArray(width * height * 4);
  const line = Buffer.alloc(stride);
  const previous = Buffer.alloc(stride);

  let source = 0;
  for (let y = 0; y < height; y += 1) {
    const filter = raw[source];
    source += 1;
    raw.copy(line, 0, source, source + stride);
    source += stride;
    unfilter(filter, line, previous, channels);
    line.copy(previous);

    for (let x = 0; x < width; x += 1) {
      const target = (y * width + x) * 4;
      const at = x * channels;
      switch (header.colourType) {
        case 0:
          pixels.set([line[at], line[at], line[at], 255], target);
          break;
        case 2:
          pixels.set([line[at], line[at + 1], line[at + 2], 255], target);
          break;
        case 3: {
          const index = line[at];
          pixels.set(
            [
              palette[index * 3],
              palette[index * 3 + 1],
              palette[index * 3 + 2],
              transparency?.[index] ?? 255,
            ],
            target,
          );
          break;
        }
        case 4:
          pixels.set([line[at], line[at], line[at], line[at + 1]], target);
          break;
        default:
          pixels.set([line[at], line[at + 1], line[at + 2], line[at + 3]], target);
      }
    }
  }

  return { width, height, pixels };
}

function unfilter(filter, line, previous, channels) {
  const paeth = (a, b, c) => {
    const p = a + b - c;
    const pa = Math.abs(p - a);
    const pb = Math.abs(p - b);
    const pc = Math.abs(p - c);
    return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
  };

  for (let i = 0; i < line.length; i += 1) {
    const left = i >= channels ? line[i - channels] : 0;
    const up = previous[i];
    const upLeft = i >= channels ? previous[i - channels] : 0;

    switch (filter) {
      case 1:
        line[i] = (line[i] + left) & 0xff;
        break;
      case 2:
        line[i] = (line[i] + up) & 0xff;
        break;
      case 3:
        line[i] = (line[i] + ((left + up) >> 1)) & 0xff;
        break;
      case 4:
        line[i] = (line[i] + paeth(left, up, upLeft)) & 0xff;
        break;
      default:
        break;
    }
  }
}

/** Redimensionnement bilinéaire avec prémultiplication alpha, sinon les bords bavent. */
export function resize(image, size) {
  const out = new Uint8ClampedArray(size * size * 4);
  const scaleX = image.width / size;
  const scaleY = image.height / size;

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      // Moyenne de la zone source couverte : evite le crenelage sur les fortes reductions.
      const x0 = Math.floor(x * scaleX);
      const x1 = Math.min(image.width, Math.max(x0 + 1, Math.ceil((x + 1) * scaleX)));
      const y0 = Math.floor(y * scaleY);
      const y1 = Math.min(image.height, Math.max(y0 + 1, Math.ceil((y + 1) * scaleY)));

      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      let count = 0;

      for (let sy = y0; sy < y1; sy += 1) {
        for (let sx = x0; sx < x1; sx += 1) {
          const at = (sy * image.width + sx) * 4;
          const alpha = image.pixels[at + 3] / 255;
          r += image.pixels[at] * alpha;
          g += image.pixels[at + 1] * alpha;
          b += image.pixels[at + 2] * alpha;
          a += alpha;
          count += 1;
        }
      }

      const target = (y * size + x) * 4;
      if (a > 0) {
        out[target] = r / a;
        out[target + 1] = g / a;
        out[target + 2] = b / a;
        out[target + 3] = (a / count) * 255;
      }
    }
  }

  return { width: size, height: size, pixels: out };
}

export function encodePng(size, pixels) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    raw[y * (size * 4 + 1)] = 0;
    Buffer.from(pixels.buffer, y * size * 4, size * 4).copy(raw, y * (size * 4 + 1) + 1);
  }

  const chunk = (type, data) => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body) >>> 0);
    return Buffer.concat([length, body, crc]);
  };

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return crc ^ 0xffffffff;
}
