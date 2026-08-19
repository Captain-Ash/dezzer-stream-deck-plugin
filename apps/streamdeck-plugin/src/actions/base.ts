/** Socle commun aux actions : accès au bridge, rafraîchissement et messages courts. */

import type { DialAction, KeyAction, WillAppearEvent } from "@elgato/streamdeck";
import { SingletonAction } from "@elgato/streamdeck";

import type { BridgeService, BridgeSnapshot } from "../bridge-service.js";
import type { Artwork } from "../key-render.js";

/** Durée d'affichage d'un message transitoire avant retour au dernier état valide (§9.3). */
const TRANSIENT_MESSAGE_MS = 1_800;

export type AnyAction = DialAction | KeyAction;

export abstract class BridgeAction extends SingletonAction {
  private readonly transientTimers = new Map<string, NodeJS.Timeout>();

  constructor(protected readonly service: BridgeService) {
    super();
  }

  /** `true` si l'action doit être réémise à haute cadence pour animer un défilement. */
  get needsAnimation(): boolean {
    return false;
  }

  hasVisibleActions(): boolean {
    for (const _ of this.actions) return true;
    return false;
  }

  /** Appelé par le plugin à chaque changement d'état du bridge. */
  refresh(snapshot: BridgeSnapshot): void {
    for (const action of this.actions) {
      if (this.transientTimers.has(action.id)) continue;
      void this.render(action, snapshot).catch(() => {
        // Un echec d'affichage ne doit jamais interrompre le plugin.
      });
    }
  }

  override async onWillAppear(event: WillAppearEvent): Promise<void> {
    await this.service.ensureRunning();
    await this.render(event.action, this.service.snapshot());
  }

  protected abstract render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void>;

  /** Pochette de la piste courante, ou `undefined` si désactivée ou indisponible. */
  protected async artwork(snapshot: BridgeSnapshot): Promise<Artwork | undefined> {
    if (!snapshot.state.available || !snapshot.state.artworkUrl) return undefined;

    const settings = await this.service.globalSettings();
    if (settings?.showArtworkOnKeys === false) return undefined;

    const dataUrl = await this.service.artworkDataUrl();
    return dataUrl ? { dataUrl } : undefined;
  }

  /** Affiche brièvement un message puis restaure l'état courant. */
  protected async flash(action: AnyAction, message: string): Promise<void> {
    if (!action.isKey()) return;

    clearTimeout(this.transientTimers.get(action.id));
    await action.setTitle(message);

    this.transientTimers.set(
      action.id,
      setTimeout(() => {
        this.transientTimers.delete(action.id);
        void this.render(action, this.service.snapshot()).catch(() => undefined);
      }, TRANSIENT_MESSAGE_MS),
    );
  }
}

/**
 * Clé du message court et actionnable décrivant pourquoi une action ne peut rien faire
 * (§13.2). Retourne `undefined` si tout est opérationnel.
 */
export function unavailableLabel(snapshot: BridgeSnapshot): string | undefined {
  switch (snapshot.status) {
    case "starting":
      return "key.starting";
    case "stopped":
      return "key.bridgeOff";
    case "failed":
      return "key.bridgeError";
    case "ready":
      return snapshot.state.available ? undefined : "key.deezerOff";
  }
}

/** Tronque sans couper au milieu d'un mot quand c'est possible. */
export function truncate(value: string, max: number): string {
  const trimmed = value.trim();
  if (trimmed.length <= max) return trimmed;
  const cut = trimmed.slice(0, max - 1);
  const lastSpace = cut.lastIndexOf(" ");
  return `${lastSpace > max / 2 ? cut.slice(0, lastSpace) : cut}…`;
}
