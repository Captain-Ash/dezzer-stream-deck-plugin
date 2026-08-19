/**
 * Rendu du widget. Tout texte issu des métadonnées passe par `textContent` : aucune
 * insertion HTML, donc aucune exécution possible d'un titre malveillant (§2.2).
 */

import {
  effectivePositionMs,
  formatDuration,
  isNewerState,
  type NowPlayingState,
} from "@dezzer/playback-contract";

import type { OverlayOptions } from "./params.js";
import { Waveform } from "./waveform.js";

/** Durée pendant laquelle on conserve l'affichage après une coupure du bridge (§10.2). */
const STALE_GRACE_MS = 8_000;

type DisplayState = "loading" | "empty" | "ready";

interface Elements {
  body: HTMLElement;
  widget: HTMLElement;
  artwork: HTMLElement;
  artworkImage: HTMLImageElement;
  title: HTMLElement;
  titleText: HTMLElement;
  artist: HTMLElement;
  artistText: HTMLElement;
  album: HTMLElement;
  albumText: HTMLElement;
  progress: HTMLElement;
  progressFill: HTMLElement;
  waveform: HTMLCanvasElement;
  times: HTMLElement;
  elapsed: HTMLElement;
  duration: HTMLElement;
  statusBadge: HTMLElement;
}

/** Vitesse de défilement d'un texte qui déborde, en pixels par seconde. */
const SCROLL_SPEED_PX_PER_S = 30;

/** Teinte des barres pas encore lues, alignée sur la piste de progression classique. */
const WAVEFORM_REMAINING_COLOUR = "rgba(255, 255, 255, 0.22)";

export class OverlayView {
  private readonly elements: Elements;
  private readonly waveform: Waveform;
  private state: NowPlayingState | undefined;
  private receivedAtMs = 0;
  private connected = false;
  private disconnectedAtMs: number | undefined;
  private hideTimer: number | undefined;
  private currentArtworkUrl: string | undefined;
  private accentColour = "#a238ff";

  constructor(
    private readonly options: OverlayOptions,
    document_: Document = document,
  ) {
    this.elements = collectElements(document_);
    this.waveform = new Waveform(this.elements.waveform);
    this.applyOptions();
  }

  private applyOptions(): void {
    const { body, widget, artwork, album, times, progress } = this.elements;

    body.dataset.theme = this.options.theme;
    widget.style.width = `${this.options.width}px`;

    if (this.options.accent) {
      widget.style.setProperty("--accent", this.options.accent);
    }

    artwork.hidden = !this.options.showArtwork;
    album.hidden = !this.options.showAlbum;
    times.hidden = !this.options.showTime;
    progress.dataset.waveform = String(this.options.waveform);

    // La couleur est relue une fois : le thème ne change plus après le chargement, et
    // interroger le style calculé à chaque image coûterait une mise en page complète.
    this.accentColour =
      this.options.accent ??
      getComputedStyle(widget).getPropertyValue("--accent").trim() ??
      this.accentColour;
  }

  /** Une trame de spectre venant du bridge. */
  pushSpectrum(bands: number[]): void {
    if (!this.options.waveform) return;
    this.waveform.push(bands);
  }

  update(state: NowPlayingState, receivedAtMs: number): void {
    if (!isNewerState(this.state, state)) return;

    const previousTrack = this.state?.trackId;
    this.state = state;
    this.receivedAtMs = receivedAtMs;
    this.disconnectedAtMs = undefined;

    if (previousTrack !== state.trackId) {
      this.animateTrackChange();
      this.waveform.reset();
    }

    this.render();
    this.scheduleAutoHide();
  }

  setConnected(connected: boolean): void {
    this.connected = connected;
    if (!connected && this.disconnectedAtMs === undefined) {
      this.disconnectedAtMs = Date.now();
    }
    this.render();
  }

  /** Appelé à chaque frame : seule la progression bouge, le reste est inchangé. */
  tick(): void {
    if (this.displayState() !== "ready") return;
    this.renderProgress();
    this.expireIfStale();
  }

  private expireIfStale(): void {
    if (this.connected || this.disconnectedAtMs === undefined) return;
    if (Date.now() - this.disconnectedAtMs > STALE_GRACE_MS) {
      this.state = undefined;
      this.render();
    }
  }

  private displayState(): DisplayState {
    if (!this.state) return this.connected ? "empty" : "loading";
    if (!this.state.available) return "empty";
    if (this.state.status === "unavailable" || this.state.status === "stopped") return "empty";
    if (!this.state.title && !this.state.artist) return "empty";
    return "ready";
  }

  private render(): void {
    const display = this.displayState();
    this.elements.body.dataset.state = display;

    if (display !== "ready" || !this.state) {
      this.setArtwork(undefined);
      return;
    }

    const state = this.state;
    this.setLine(this.elements.title, this.elements.titleText, state.title ?? "");
    this.setLine(this.elements.artist, this.elements.artistText, state.artist ?? "");
    this.setLine(this.elements.album, this.elements.albumText, state.album ?? "");
    this.elements.album.hidden = !this.options.showAlbum || !state.album;
    this.elements.statusBadge.dataset.status = state.status;

    this.setArtwork(state.artworkUrl);
    this.renderProgress();
  }

  /**
   * Un texte plus large que sa ligne défile en aller-retour.
   *
   * La mesure est synchrone : lire `scrollWidth` force le calcul de mise en page, alors
   * qu'un `requestAnimationFrame` ne se déclencherait pas si la page est masquée.
   */
  private setLine(line: HTMLElement, text: HTMLElement, value: string): void {
    if (text.textContent !== value) {
      text.textContent = value;
    }

    const overflow = text.scrollWidth - line.clientWidth;
    if (overflow <= 1) {
      line.dataset.overflow = "false";
      line.style.removeProperty("--scroll-distance");
      line.style.removeProperty("--scroll-duration");
      return;
    }

    // La cinematique CSS consacre 76 % du cycle au deplacement, le reste aux pauses.
    const travelSeconds = overflow / SCROLL_SPEED_PX_PER_S;
    line.dataset.overflow = "true";
    line.style.setProperty("--scroll-distance", `${-overflow}px`);
    line.style.setProperty("--scroll-duration", `${(travelSeconds / 0.38).toFixed(2)}s`);
  }

  private renderProgress(): void {
    const state = this.state;
    if (!state) return;

    const position = effectivePositionMs(state, this.receivedAtMs);
    const duration = state.durationMs;
    const hasProgress = position !== undefined && duration !== undefined && duration > 0;

    this.elements.progress.hidden = position === undefined;
    this.elements.progressFill.style.width = hasProgress
      ? `${Math.min(100, (position / duration) * 100)}%`
      : "0%";

    if (this.options.waveform) {
      this.waveform.draw(
        hasProgress ? Math.min(1, position / duration) : 0,
        this.accentColour,
        WAVEFORM_REMAINING_COLOUR,
      );
    }

    this.elements.elapsed.textContent = formatDuration(position);
    this.elements.duration.textContent = formatDuration(duration);
  }

  /**
   * Les pochettes sont servies par le bridge : le token est ajouté ici, jamais dans
   * l'état diffusé.
   */
  private setArtwork(url: string | undefined): void {
    const resolved = url ? `${url}?token=${encodeURIComponent(this.options.token)}` : undefined;
    if (resolved === this.currentArtworkUrl) return;

    this.currentArtworkUrl = resolved;
    const image = this.elements.artworkImage;

    if (!resolved) {
      image.removeAttribute("src");
      this.elements.artwork.dataset.loaded = "false";
      return;
    }

    image.onload = () => {
      this.elements.artwork.dataset.loaded = "true";
    };
    image.onerror = () => {
      image.removeAttribute("src");
      this.elements.artwork.dataset.loaded = "false";
    };
    image.src = resolved;
  }

  private animateTrackChange(): void {
    const widget = this.elements.widget;
    widget.classList.remove("track-change");
    // Force un reflow pour rejouer l'animation sur une piste enchainee.
    void widget.offsetWidth;
    widget.classList.add("track-change");
  }

  private scheduleAutoHide(): void {
    if (this.hideTimer !== undefined) {
      window.clearTimeout(this.hideTimer);
      this.hideTimer = undefined;
    }
    if (!this.options.autoHide) {
      this.elements.body.dataset.hidden = "false";
      return;
    }

    this.elements.body.dataset.hidden = "false";
    this.hideTimer = window.setTimeout(() => {
      this.elements.body.dataset.hidden = "true";
    }, this.options.hideAfterMs);
  }
}

function collectElements(document_: Document): Elements {
  const require_ = <T extends HTMLElement>(id: string): T => {
    const element = document_.getElementById(id);
    if (!element) throw new Error(`element introuvable : ${id}`);
    return element as T;
  };

  return {
    body: document_.body,
    widget: require_("widget"),
    artwork: require_("artwork"),
    artworkImage: require_<HTMLImageElement>("artwork-image"),
    title: require_("title"),
    titleText: require_("title-text"),
    artist: require_("artist"),
    artistText: require_("artist-text"),
    album: require_("album"),
    albumText: require_("album-text"),
    progress: require_("progress"),
    progressFill: require_("progress-fill"),
    waveform: require_<HTMLCanvasElement>("waveform"),
    times: require_("times"),
    elapsed: require_("elapsed"),
    duration: require_("duration"),
    statusBadge: require_("status-badge"),
  };
}
