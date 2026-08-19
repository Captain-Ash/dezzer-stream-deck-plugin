import { cp, mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = dirname(fileURLToPath(import.meta.url));
const outDir = resolve(root, "dist");

await rm(outDir, { recursive: true, force: true });
await mkdir(outDir, { recursive: true });

await build({
  entryPoints: [resolve(root, "src/main.ts")],
  outfile: resolve(outDir, "assets/overlay.js"),
  bundle: true,
  format: "esm",
  target: "chrome110",
  minify: true,
  sourcemap: false,
  // Aucun CDN, aucune dependance externe : tout est inline dans le bundle.
  external: [],
  logLevel: "info",
});

await cp(resolve(root, "public"), outDir, { recursive: true });

console.log(`overlay compile dans ${outDir}`);
