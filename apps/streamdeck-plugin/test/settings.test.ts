import assert from "node:assert/strict";
import { test } from "node:test";

import { isContractCompatible } from "../src/bridge-manager.js";
import { buildOverlayUrl, normaliseOverlay } from "../src/settings.js";

test("refuse un bridge dont le contrat est incompatible", () => {
  assert.equal(isContractCompatible("1.0.0"), true);
  assert.equal(isContractCompatible("1.7.3"), true);
  assert.equal(isContractCompatible("2.0.0"), false);
  assert.equal(isContractCompatible("0.9.0"), false);
  assert.equal(isContractCompatible("n'importe quoi"), false);
});

test("normalise les reglages d'overlay hors bornes ou hostiles", () => {
  const settings = normaliseOverlay({
    theme: "pwned" as never,
    width: 99_999,
    hideAfterMs: 1,
    accent: "red; background: url(http://evil)",
  });

  assert.equal(settings.theme, "glass");
  assert.equal(settings.width, 1200);
  assert.equal(settings.hideAfterMs, 1_000);
  assert.equal(settings.accent, "", "une couleur non hexadecimale est rejetee");
});

test("conserve une configuration valide", () => {
  const settings = normaliseOverlay({ theme: "neon", width: 800, accent: "#ff0066" });
  assert.equal(settings.theme, "neon");
  assert.equal(settings.width, 800);
  assert.equal(settings.accent, "#ff0066");
});

test("construit une URL d'overlay loopback contenant le jeton", () => {
  const url = new URL(buildOverlayUrl(53211, "s3cr3t", normaliseOverlay({ theme: "neon" })));

  assert.equal(url.protocol, "http:");
  assert.equal(url.hostname, "127.0.0.1");
  assert.equal(url.port, "53211");
  assert.equal(url.pathname, "/overlay/");
  assert.equal(url.searchParams.get("token"), "s3cr3t");
  assert.equal(url.searchParams.get("theme"), "neon");
  assert.equal(url.searchParams.get("showTime"), "1");
});

test("n'ajoute le delai de masquage que si le masquage auto est actif", () => {
  const off = new URL(buildOverlayUrl(1, "t", normaliseOverlay({ autoHide: false })));
  assert.equal(off.searchParams.has("hideAfterMs"), false);

  const on = new URL(buildOverlayUrl(1, "t", normaliseOverlay({ autoHide: true, hideAfterMs: 4_000 })));
  assert.equal(on.searchParams.get("hideAfterMs"), "4000");
});
