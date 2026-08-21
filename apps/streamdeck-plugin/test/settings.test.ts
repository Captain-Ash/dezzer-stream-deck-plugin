import assert from "node:assert/strict";
import { test } from "node:test";

import { isContractCompatible } from "../src/bridge-manager.js";
import { normaliseVolumeStep } from "../src/settings.js";

test("refuse un bridge dont le contrat est incompatible", () => {
  assert.equal(isContractCompatible("1.0.0"), true);
  assert.equal(isContractCompatible("1.7.3"), true);
  assert.equal(isContractCompatible("2.0.0"), false);
  assert.equal(isContractCompatible("0.9.0"), false);
  assert.equal(isContractCompatible("n'importe quoi"), false);
});

test("ramene un pas de volume invalide sur la valeur par defaut", () => {
  assert.equal(normaliseVolumeStep(10), 10);
  assert.equal(normaliseVolumeStep("2"), 2);
  assert.equal(normaliseVolumeStep(7), 5);
  assert.equal(normaliseVolumeStep(undefined), 5);
  assert.equal(normaliseVolumeStep("n'importe quoi"), 5);
});
