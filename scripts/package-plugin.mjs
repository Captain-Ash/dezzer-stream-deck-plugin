/**
 * Assemble le paquet Stream Deck : icônes, overlay compilé, bundle du plugin et binaire
 * du bridge, tous réunis dans `com.dezzer.deezer.sdPlugin`.
 *
 * Usage : `node scripts/package-plugin.mjs [--debug]`
 */

import { spawnSync } from "node:child_process";
import { cp, mkdir, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sdPlugin = join(root, "apps/streamdeck-plugin/com.dezzer.deezer.sdPlugin");
const release = !process.argv.includes("--debug");

function run(command, args, cwd = root, extraEnv = {}) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd,
    stdio: "inherit",
    shell: process.platform === "win32",
    env: { ...process.env, ...extraEnv },
  });
  if (result.status !== 0) {
    throw new Error(`${command} a echoue (code ${result.status})`);
  }
}

/**
 * rustc embarque le chemin absolu des sources (y compris celles des dependances mises en
 * cache par cargo, ex. `C:\Users\<compte>\.cargo\registry\src\...`) dans les messages de
 * panique du binaire compile. On le remplace par un chemin virtuel stable pour que le
 * binaire livre ne contienne ni le nom de compte ni la machine qui l'a compile.
 */
function buildRustflags() {
  const cargoHome = process.env.CARGO_HOME || join(homedir(), ".cargo");
  const remaps = [
    `--remap-path-prefix=${join(cargoHome, "registry", "src")}=/cargo/registry/src`,
    `--remap-path-prefix=${join(cargoHome, "git", "checkouts")}=/cargo/git/checkouts`,
    `--remap-path-prefix=${root}=/dezzer`,
  ];
  return [process.env.RUSTFLAGS, ...remaps].filter(Boolean).join(" ");
}

async function exists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

console.log("=== 1/4 icones ===");
run("node", ["scripts/generate-icons.mjs"]);

console.log("\n=== 2/4 overlay ===");
run("npm", ["run", "build", "-w", "@dezzer/overlay"]);
const overlayTarget = join(sdPlugin, "overlay");
await rm(overlayTarget, { recursive: true, force: true });
await cp(join(root, "apps/overlay/dist"), overlayTarget, { recursive: true });

console.log("\n=== 3/4 plugin ===");
run("npm", ["run", "build", "-w", "@dezzer/streamdeck-plugin"]);

console.log("\n=== 4/4 bridge ===");
const cargoArgs = ["build", "--manifest-path", "apps/bridge/Cargo.toml"];
if (release) cargoArgs.push("--release");
run("cargo", cargoArgs, root, { RUSTFLAGS: buildRustflags() });

const profile = release ? "release" : "debug";
const binaryName = process.platform === "win32" ? "dezzer-bridge.exe" : "dezzer-bridge";
const builtBinary = join(root, "apps/bridge/target", profile, binaryName);

if (!(await exists(builtBinary))) {
  throw new Error(`binaire introuvable : ${builtBinary}`);
}

const binDir = join(sdPlugin, "bin", `${process.platform}-${process.arch}`);
await mkdir(binDir, { recursive: true });
await cp(builtBinary, join(binDir, binaryName));

const { size } = await stat(join(binDir, binaryName));
console.log(`\nbridge : ${(size / 1024 / 1024).toFixed(2)} Mo -> ${binDir}`);
console.log(`paquet pret : ${sdPlugin}`);
console.log(
  "\nPour l'installer : copiez ce dossier dans %APPDATA%\\Elgato\\StreamDeck\\Plugins " +
    "puis redemarrez Stream Deck (ou utilisez `npm run plugin:install`).",
);
