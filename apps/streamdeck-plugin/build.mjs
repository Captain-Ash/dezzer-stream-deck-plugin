import { build } from "esbuild";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const outfile = resolve(root, "com.dezzer.deezer.sdPlugin/bin/plugin.js");

await build({
  entryPoints: [resolve(root, "src/plugin.ts")],
  outfile,
  bundle: true,
  platform: "node",
  target: "node20",
  // Stream Deck lance `node bin/plugin.js` sans package.json adjacent : le CommonJS
  // evite toute ambiguite de resolution de modules.
  format: "cjs",
  sourcemap: true,
  minify: false,
  logLevel: "info",
});

console.log(`plugin compile dans ${outfile}`);
