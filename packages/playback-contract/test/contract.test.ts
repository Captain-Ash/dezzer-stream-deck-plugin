import assert from "node:assert/strict";
import { test } from "node:test";

import {
  effectivePositionMs,
  formatDuration,
  isNewerState,
  NO_CAPABILITIES,
  SCHEMA_VERSION,
  type NowPlayingState,
} from "../src/index.js";

const base: NowPlayingState = {
  schemaVersion: SCHEMA_VERSION,
  source: "deezer-desktop",
  available: true,
  status: "playing",
  positionMs: 30_000,
  durationMs: 200_000,
  capabilities: NO_CAPABILITIES,
  updatedAt: "2026-08-19T13:20:00.000Z",
  sequence: 10,
};

test("extrapole la position uniquement pendant la lecture", () => {
  assert.equal(effectivePositionMs(base, 1_000, 3_500), 32_500);
  assert.equal(effectivePositionMs({ ...base, status: "paused" }, 1_000, 3_500), 30_000);
});

test("borne la position extrapolee a la duree", () => {
  assert.equal(effectivePositionMs(base, 0, 500_000), 200_000);
});

test("ne fabrique pas de position quand elle est inconnue", () => {
  const withoutPosition: NowPlayingState = { ...base, positionMs: undefined };
  assert.equal(effectivePositionMs(withoutPosition, 0, 1_000), undefined);
});

test("ignore un evenement plus ancien que l'etat courant", () => {
  assert.equal(isNewerState(base, { ...base, sequence: 9 }), false);
  assert.equal(isNewerState(base, { ...base, sequence: 10 }), true);
  assert.equal(isNewerState(base, { ...base, sequence: 11 }), true);
  assert.equal(isNewerState(undefined, base), true);
});

test("formate les durees et signale l'absence de donnee", () => {
  assert.equal(formatDuration(0), "0:00");
  assert.equal(formatDuration(65_000), "1:05");
  assert.equal(formatDuration(3_725_000), "1:02:05");
  assert.equal(formatDuration(undefined), "--:--");
  assert.equal(formatDuration(-1), "--:--");
  assert.equal(formatDuration(Number.NaN), "--:--");
});
