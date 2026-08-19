import assert from "node:assert/strict";
import { test } from "node:test";

import { NO_CAPABILITIES, SCHEMA_VERSION, type NowPlayingState } from "@dezzer/playback-contract";

import { truncate, unavailableLabel } from "../src/actions/base.js";
import type { BridgeSnapshot } from "../src/bridge-service.js";

const state: NowPlayingState = {
  schemaVersion: SCHEMA_VERSION,
  source: "deezer-desktop",
  available: true,
  status: "playing",
  title: "Un titre",
  artist: "Un artiste",
  positionMs: 83_000,
  durationMs: 200_000,
  capabilities: { ...NO_CAPABILITIES, playPause: true, next: true, previous: true },
  updatedAt: "2026-08-19T13:20:00.000Z",
  sequence: 4,
};

function snapshot(overrides: Partial<BridgeSnapshot> = {}): BridgeSnapshot {
  return {
    status: "ready",
    state,
    receivedAtMs: Date.now(),
    info: undefined,
    lastError: undefined,
    ...overrides,
  };
}

test("annonce un motif court et actionnable quand rien n'est disponible", () => {
  assert.equal(unavailableLabel(snapshot()), undefined);
  assert.equal(unavailableLabel(snapshot({ status: "starting" })), "key.starting");
  assert.equal(unavailableLabel(snapshot({ status: "stopped" })), "key.bridgeOff");
  assert.equal(unavailableLabel(snapshot({ status: "failed" })), "key.bridgeError");
  assert.equal(
    unavailableLabel(snapshot({ state: { ...state, available: false } })),
    "key.deezerOff",
  );
});

test("tronque sans couper au milieu d'un mot quand c'est possible", () => {
  assert.equal(truncate("court", 16), "court");
  assert.equal(truncate("Collectif Débordement et invités", 16), "Collectif…");
  assert.equal(truncate("Anticonstitutionnellement", 16), "Anticonstitutio…");
});
