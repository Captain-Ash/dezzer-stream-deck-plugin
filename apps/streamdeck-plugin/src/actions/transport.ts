import { action, type KeyDownEvent } from "@elgato/streamdeck";

import type { BridgeSnapshot } from "../bridge-service.js";
import { t } from "../i18n.js";
import { glyphKey, unavailableKey } from "../key-render.js";
import { normaliseVolumeStep } from "../settings.js";
import { BridgeAction, unavailableLabel, type AnyAction } from "./base.js";

@action({ UUID: "com.dezzer.deezer.play-pause" })
export class PlayPauseAction extends BridgeAction {
  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    const failure = await this.service.command("play-pause");
    if (failure) {
      await event.action.showAlert();
      await this.flash(event.action, failure);
    }
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const label = unavailableLabel(snapshot);
    if (label) {
      await action.setImage(unavailableKey("play", t(label)));
      await action.setTitle("");
      return;
    }

    if (!snapshot.state.capabilities.playPause) {
      await action.setImage(unavailableKey("play", t("key.unsupported")));
      await action.setTitle("");
      return;
    }

    const playing = snapshot.state.status === "playing";
    await action.setImage(
      glyphKey({
        glyph: playing ? "pause" : "play",
        mood: playing ? "active" : "idle",
        artwork: await this.artwork(snapshot),
      }),
    );
    await action.setTitle("");
  }
}

@action({ UUID: "com.dezzer.deezer.next" })
export class NextAction extends BridgeAction {
  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    const failure = await this.service.command("next");
    if (failure) {
      await event.action.showAlert();
      await this.flash(event.action, failure);
    }
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const label = unavailableLabel(snapshot);
    if (label) {
      await action.setImage(unavailableKey("next", t(label)));
    } else if (!snapshot.state.capabilities.next) {
      await action.setImage(unavailableKey("next", t("key.unsupported")));
    } else {
      await action.setImage(glyphKey({ glyph: "next", mood: "idle" }));
    }
    await action.setTitle("");
  }
}

@action({ UUID: "com.dezzer.deezer.previous" })
export class PreviousAction extends BridgeAction {
  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    const failure = await this.service.command("previous");
    if (failure) {
      await event.action.showAlert();
      await this.flash(event.action, failure);
    }
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const label = unavailableLabel(snapshot);
    if (label) {
      await action.setImage(unavailableKey("previous", t(label)));
    } else if (!snapshot.state.capabilities.previous) {
      await action.setImage(unavailableKey("previous", t("key.unsupported")));
    } else {
      await action.setImage(glyphKey({ glyph: "previous", mood: "idle" }));
    }
    await action.setTitle("");
  }
}

abstract class VolumeAction extends BridgeAction {
  protected abstract readonly direction: 1 | -1;
  protected abstract readonly glyph: "volume-up" | "volume-down";

  override async onKeyDown(event: KeyDownEvent): Promise<void> {
    const snapshot = this.service.snapshot();
    if (!snapshot.state.capabilities.volume || snapshot.state.volume === undefined) {
      await event.action.showAlert();
      await this.flash(event.action, t("key.unavailable"));
      return;
    }

    const settings = await this.service.globalSettings();
    const step = normaliseVolumeStep(settings?.volumeStep) * this.direction;
    const target = Math.min(100, Math.max(0, snapshot.state.volume + step));

    const failure = await this.service.setVolume(target);
    if (failure) {
      await event.action.showAlert();
      await this.flash(event.action, failure);
    }
  }

  protected override async render(action: AnyAction, snapshot: BridgeSnapshot): Promise<void> {
    if (!action.isKey()) return;

    const label = unavailableLabel(snapshot);
    if (label) {
      await action.setImage(unavailableKey(this.glyph, t(label)));
      await action.setTitle("");
      return;
    }

    if (!snapshot.state.capabilities.volume) {
      await action.setImage(unavailableKey(this.glyph, t("key.unavailableShort")));
      await action.setTitle("");
      return;
    }

    await action.setImage(glyphKey({ glyph: this.glyph, mood: "idle" }));
    await action.setTitle(snapshot.state.volume === undefined ? "" : `${snapshot.state.volume}%`);
  }
}

@action({ UUID: "com.dezzer.deezer.volume-up" })
export class VolumeUpAction extends VolumeAction {
  protected override readonly direction = 1 as const;
  protected override readonly glyph = "volume-up" as const;
}

@action({ UUID: "com.dezzer.deezer.volume-down" })
export class VolumeDownAction extends VolumeAction {
  protected override readonly direction = -1 as const;
  protected override readonly glyph = "volume-down" as const;
}
