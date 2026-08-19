/**
 * Cycle de vie du binaire compagnon.
 *
 * L'utilisateur ne lance jamais rien : le plugin démarre le bridge, le surveille, le
 * relance un nombre limité de fois et l'arrête à la fermeture (§9.4).
 */

import { type ChildProcess, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

import streamDeck from "@elgato/streamdeck";

export type BridgeStatus = "stopped" | "starting" | "ready" | "failed";

export interface BridgeConnectionInfo {
  port: number;
  token: string;
  version: string;
  contractVersion: string;
  adapter: string;
}

/** Version de contrat maximale comprise par ce plugin. */
const SUPPORTED_CONTRACT_MAJOR = 1;

const HEALTH_TIMEOUT_MS = 15_000;
const HEALTH_BACKOFF_MS = [100, 150, 250, 400, 600, 800, 1_000];

/** Au-delà, on cesse de relancer pour ne pas boucler indéfiniment. */
const MAX_RESTARTS = 3;
const RESTART_WINDOW_MS = 60_000;

export class BridgeManager {
  private child: ChildProcess | undefined;
  private info: BridgeConnectionInfo | undefined;
  private status: BridgeStatus = "stopped";
  private starting: Promise<BridgeConnectionInfo> | undefined;
  private restarts: number[] = [];
  private lastError: string | undefined;
  private readonly listeners = new Set<(status: BridgeStatus) => void>();

  /**
   * Le jeton est fourni par une fonction : il vit dans les réglages globaux, qui ne sont
   * lisibles qu'une fois la connexion à Stream Deck établie.
   */
  constructor(
    private readonly pluginRoot: string,
    private readonly tokenProvider: () => Promise<string>,
  ) {}

  onStatusChange(listener: (status: BridgeStatus) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getStatus(): BridgeStatus {
    return this.status;
  }

  getInfo(): BridgeConnectionInfo | undefined {
    return this.info;
  }

  getLastError(): string | undefined {
    return this.lastError;
  }

  async ensureRunning(): Promise<BridgeConnectionInfo> {
    if (this.status === "ready" && this.info) return this.info;
    this.starting ??= this.start().finally(() => {
      this.starting = undefined;
    });
    return this.starting;
  }

  async reconnect(): Promise<BridgeConnectionInfo> {
    this.restarts = [];
    await this.stop();
    return this.ensureRunning();
  }

  async stop(): Promise<void> {
    const child = this.child;
    this.child = undefined;
    this.info = undefined;
    this.setStatus("stopped");

    if (!child || child.exitCode !== null) return;

    child.kill();
    await new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        resolve();
      }, 2_000);
      child.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }

  private setStatus(status: BridgeStatus): void {
    if (this.status === status) return;
    this.status = status;
    for (const listener of this.listeners) listener(status);
  }

  private async start(): Promise<BridgeConnectionInfo> {
    this.setStatus("starting");

    const binary = this.resolveBinary();
    if (!binary) {
      this.lastError = "Le composant local est absent du plugin.";
      this.setStatus("failed");
      throw new Error(this.lastError);
    }

    const token = await this.tokenProvider();

    const child = spawn(binary, [], {
      env: {
        ...process.env,
        DEZZER_BRIDGE_TOKEN: token,
        DEZZER_BRIDGE_PARENT_PID: String(process.pid),
        DEZZER_BRIDGE_OVERLAY_DIR: join(this.pluginRoot, "overlay"),
      },
      stdio: "ignore",
      windowsHide: true,
    });

    this.child = child;

    child.once("error", (error) => {
      this.lastError = error.message;
      streamDeck.logger.error("echec de lancement du bridge", error);
      this.setStatus("failed");
    });

    child.once("exit", (code) => {
      if (this.child !== child) return;
      streamDeck.logger.warn(`bridge termine (code ${code})`);
      this.child = undefined;
      this.info = undefined;
      this.setStatus("stopped");
      void this.restartIfAllowed();
    });

    const info = await this.waitForHealth(token, child.pid);
    this.info = info;
    this.lastError = undefined;
    this.setStatus("ready");
    streamDeck.logger.info(
      `bridge pret port=${info.port} version=${info.version} adapter=${info.adapter}`,
    );
    return info;
  }

  private async restartIfAllowed(): Promise<void> {
    const now = Date.now();
    this.restarts = this.restarts.filter((at) => now - at < RESTART_WINDOW_MS);

    if (this.restarts.length >= MAX_RESTARTS) {
      this.lastError = "Le composant local redémarre en boucle.";
      this.setStatus("failed");
      streamDeck.logger.error("trop de redemarrages du bridge, abandon");
      return;
    }

    this.restarts.push(now);
    await new Promise((resolve) => setTimeout(resolve, 1_000));
    try {
      await this.ensureRunning();
    } catch (error) {
      streamDeck.logger.error("relance du bridge impossible", error);
    }
  }

  private resolveBinary(): string | undefined {
    const platform = process.platform;
    const arch = process.arch;
    const name = platform === "win32" ? "dezzer-bridge.exe" : "dezzer-bridge";
    const candidates = [
      join(this.pluginRoot, "bin", `${platform}-${arch}`, name),
      join(this.pluginRoot, "bin", name),
    ];
    return candidates.find((candidate) => existsSync(candidate));
  }

  /**
   * Le port est éphémère : on le découvre par le fichier de disponibilité écrit par le
   * bridge, puis on interroge `/health` jusqu'à `ready`.
   *
   * Stream Deck exécute ses plugins dans un job object Windows : à la fermeture, le bridge
   * est tué net et ne peut pas nettoyer son fichier de disponibilité. On exige donc que le
   * PID qui y figure soit bien celui du processus qu'on vient de lancer, sinon on lirait le
   * port d'une session précédente.
   */
  private async waitForHealth(
    token: string,
    childPid: number | undefined,
  ): Promise<BridgeConnectionInfo> {
    const deadline = Date.now() + HEALTH_TIMEOUT_MS;
    let attempt = 0;

    while (Date.now() < deadline) {
      const runtime = await readRuntimeInfo();
      if (runtime && (childPid === undefined || runtime.pid === childPid)) {
        const health = await probeHealth(runtime.port, token);
        if (health) {
          if (!isContractCompatible(health.contractVersion)) {
            this.lastError = `Composant local incompatible (contrat ${health.contractVersion}).`;
            this.setStatus("failed");
            throw new Error(this.lastError);
          }
          return { port: runtime.port, token, ...health };
        }
      }

      const delay = HEALTH_BACKOFF_MS[Math.min(attempt, HEALTH_BACKOFF_MS.length - 1)] ?? 1_000;
      attempt += 1;
      await new Promise((resolve) => setTimeout(resolve, delay));
    }

    this.lastError = "Le composant local n'a pas démarré à temps.";
    this.setStatus("failed");
    throw new Error(this.lastError);
  }
}

export function isContractCompatible(contractVersion: string): boolean {
  const major = Number.parseInt(contractVersion.split(".")[0] ?? "", 10);
  return Number.isFinite(major) && major === SUPPORTED_CONTRACT_MAJOR;
}

export function runtimeFilePath(): string {
  const base =
    process.env.LOCALAPPDATA ??
    process.env.XDG_DATA_HOME ??
    join(process.env.HOME ?? process.env.USERPROFILE ?? ".", ".local", "share");
  return join(base, "Dezzer", "bridge-runtime.json");
}

async function readRuntimeInfo(): Promise<{ pid: number; port: number } | undefined> {
  try {
    const { readFile } = await import("node:fs/promises");
    const raw = await readFile(runtimeFilePath(), "utf8");
    const parsed = JSON.parse(raw) as { pid?: number; port?: number };
    if (typeof parsed.pid !== "number" || typeof parsed.port !== "number") return undefined;
    return parsed.port > 0 ? { pid: parsed.pid, port: parsed.port } : undefined;
  } catch {
    return undefined;
  }
}

async function probeHealth(
  port: number,
  token: string,
): Promise<Omit<BridgeConnectionInfo, "port" | "token"> | undefined> {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/health`, {
      headers: { Authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(2_000),
    });
    if (!response.ok) return undefined;

    const body = (await response.json()) as {
      ready?: boolean;
      version?: string;
      contractVersion?: string;
      adapter?: string;
    };
    if (!body.ready) return undefined;

    return {
      version: body.version ?? "?",
      contractVersion: body.contractVersion ?? "0.0.0",
      adapter: body.adapter ?? "?",
    };
  } catch {
    return undefined;
  }
}
