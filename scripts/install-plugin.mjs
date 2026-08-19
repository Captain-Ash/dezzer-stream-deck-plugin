/**
 * Installe le paquet construit dans Stream Deck.
 *
 * Copie `com.dezzer.deezer.sdPlugin` dans le dossier Plugins de Stream Deck, en arrêtant
 * puis relançant l'application pour qu'elle recharge le plugin.
 */

import { spawnSync } from "node:child_process";
import { cp, mkdir, rm, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "apps/streamdeck-plugin/com.dezzer.deezer.sdPlugin");

if (process.platform !== "win32") {
  console.error("Ce script ne gere que Windows pour le moment.");
  process.exit(1);
}

try {
  await stat(join(source, "bin", "plugin.js"));
} catch {
  console.error("Le paquet n'est pas construit. Lancez d'abord : npm run build");
  process.exit(1);
}

const pluginsDir = join(process.env.APPDATA ?? "", "Elgato", "StreamDeck", "Plugins");
const target = join(pluginsDir, "com.dezzer.deezer.sdPlugin");

const streamDeckExe = spawnSync("powershell", [
  "-NoProfile",
  "-Command",
  "(Get-Process StreamDeck -ErrorAction SilentlyContinue | Select-Object -First 1).Path",
])
  .stdout.toString()
  .trim();

if (streamDeckExe) {
  console.log("arret de Stream Deck…");
  spawnSync("powershell", [
    "-NoProfile",
    "-Command",
    "Stop-Process -Name StreamDeck -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2",
  ]);
}

// Le dossier cible n'est produit que par ce script : sa suppression est sans risque.
await rm(target, { recursive: true, force: true });
await mkdir(pluginsDir, { recursive: true });
await cp(source, target, { recursive: true });
console.log(`plugin installe : ${target}`);

if (streamDeckExe) {
  console.log("relance de Stream Deck…");
  spawnSync("powershell", [
    "-NoProfile",
    "-Command",
    `Start-Process '${streamDeckExe.replace(/'/g, "''")}'`,
  ]);
}
