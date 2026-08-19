import assert from "node:assert/strict";
import { test } from "node:test";

import { escapeXml, estimateTextWidth, nowPlayingKey, scrollOffset } from "../src/key-render.js";
import { normaliseVolumeStep } from "../src/settings.js";

test("le texte reste immobile quand il tient dans la touche", () => {
  assert.equal(scrollOffset(0, 0), 0);
  assert.equal(scrollOffset(-10, 5_000), 0);
});

test("le defilement fait un aller-retour complet et revient a zero", () => {
  const overflow = 60;
  // Pause initiale.
  assert.equal(scrollOffset(overflow, 0), 0);
  assert.equal(scrollOffset(overflow, 1_000), 0);

  // Aller : le decalage devient negatif sans jamais depasser le debordement.
  const middle = scrollOffset(overflow, 2_400);
  assert.ok(middle < 0 && middle > -overflow, `decalage inattendu : ${middle}`);

  // Le cycle est periodique : meme instant d'un cycle a l'autre, meme decalage.
  const travelMs = (overflow / 26) * 1_000;
  const cycleMs = travelMs * 2 + 2_400;
  assert.equal(scrollOffset(overflow, 2_400), scrollOffset(overflow, 2_400 + cycleMs));
});

test("le decalage ne depasse jamais le debordement", () => {
  const overflow = 40;
  for (let t = 0; t < 30_000; t += 97) {
    const offset = scrollOffset(overflow, t);
    assert.ok(offset <= 0 && offset >= -overflow, `t=${t} decalage=${offset}`);
  }
});

test("estime une largeur croissante avec la longueur et la graisse", () => {
  assert.ok(estimateTextWidth("abcdef", 19) > estimateTextWidth("abc", 19));
  assert.ok(estimateTextWidth("abc", 19, true) > estimateTextWidth("abc", 19, false));
  assert.equal(estimateTextWidth("", 19), 0);
});

test("echappe les metadonnees hostiles avant de les dessiner", () => {
  assert.equal(escapeXml("<script>"), "&lt;script&gt;");
  assert.equal(escapeXml('a & "b"'), "a &amp; &quot;b&quot;");
  assert.equal(escapeXml("O'Brien"), "O&apos;Brien");
});

test("l'image de touche est une data URL SVG sans balise injectee", () => {
  const image = nowPlayingKey({
    title: "<script>alert(1)</script>",
    artist: "Injection & Co",
    time: "1:23",
    playing: true,
    elapsedMs: 0,
  });

  assert.ok(image.startsWith("data:image/svg+xml;base64,"));
  const svg = Buffer.from(image.slice("data:image/svg+xml;base64,".length), "base64").toString();
  assert.ok(!svg.includes("<script>"), "le titre hostile ne doit pas produire de balise");
  assert.ok(svg.includes("&lt;script&gt;"));
});

test("n'accepte que les pas de volume proposes", () => {
  assert.equal(normaliseVolumeStep(1), 1);
  assert.equal(normaliseVolumeStep("10"), 10);
  assert.equal(normaliseVolumeStep(7), 5);
  assert.equal(normaliseVolumeStep(undefined), 5);
  assert.equal(normaliseVolumeStep("beaucoup"), 5);
});
