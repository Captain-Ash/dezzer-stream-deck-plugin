import assert from "node:assert/strict";
import { test } from "node:test";

import { parseAccent, parseBoundedInt, parseOptions, parseTheme } from "../src/params.js";

test("applique les valeurs par defaut de la specification", () => {
  const options = parseOptions("");
  assert.equal(options.theme, "glass");
  assert.equal(options.width, 720);
  assert.equal(options.showAlbum, false);
  assert.equal(options.showTime, true);
  assert.equal(options.showArtwork, true);
  assert.equal(options.waveform, false);
  assert.equal(options.autoHide, false);
  assert.equal(options.hideAfterMs, 10_000);
  assert.equal(options.accent, undefined);
});

test("lit une configuration complete", () => {
  const options = parseOptions(
    "?token=abc&theme=neon&width=900&showAlbum=1&showTime=0&waveform=1&autoHide=1&hideAfterMs=5000&accent=%23ff0066",
  );
  assert.equal(options.token, "abc");
  assert.equal(options.theme, "neon");
  assert.equal(options.width, 900);
  assert.equal(options.showAlbum, true);
  assert.equal(options.showTime, false);
  assert.equal(options.waveform, true);
  assert.equal(options.autoHide, true);
  assert.equal(options.hideAfterMs, 5_000);
  assert.equal(options.accent, "#ff0066");
});

test("rejette un theme inconnu au lieu de l'utiliser", () => {
  assert.equal(parseTheme("../../etc/passwd"), "glass");
  assert.equal(parseTheme("<script>"), "glass");
  assert.equal(parseTheme(null), "glass");
  assert.equal(parseTheme(" NEON "), "neon");
});

test("borne les valeurs numeriques hors plage", () => {
  const range = { min: 400, max: 1200 };
  assert.equal(parseBoundedInt("99999", 720, range), 1200);
  assert.equal(parseBoundedInt("-5", 720, range), 400);
  assert.equal(parseBoundedInt("pas-un-nombre", 720, range), 720);
  assert.equal(parseBoundedInt(null, 720, range), 720);
});

test("n'accepte qu'une couleur hexadecimale stricte", () => {
  assert.equal(parseAccent("#abc"), "#abc");
  assert.equal(parseAccent("#A1B2C3"), "#A1B2C3");
  assert.equal(parseAccent("#a1b2c3ff"), "#a1b2c3ff");
  // Toute tentative d'injection CSS doit etre ignoree.
  assert.equal(parseAccent("red; background: url(http://evil)"), undefined);
  assert.equal(parseAccent("expression(alert(1))"), undefined);
  assert.equal(parseAccent("#12"), undefined);
  assert.equal(parseAccent("javascript:alert(1)"), undefined);
});

test("ne fait jamais confiance a une valeur booleenne arbitraire", () => {
  const options = parseOptions("?showArtwork=oui&autoHide=%3Cscript%3E");
  assert.equal(options.showArtwork, true, "retombe sur le defaut");
  assert.equal(options.autoHide, false, "retombe sur le defaut");
});
