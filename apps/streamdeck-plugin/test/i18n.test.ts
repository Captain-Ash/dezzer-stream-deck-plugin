import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const sdPlugin = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "com.dezzer.deezer.sdPlugin",
);

const read = (file: string) =>
  JSON.parse(readFileSync(join(sdPlugin, file), "utf8")) as Record<string, unknown>;

const en = read("en.json");
const fr = read("fr.json");
const manifest = read("manifest.json") as { Actions: { UUID: string }[] };

/**
 * Le SDK résout les clés par chemin (`get()` découpe sur les points) : les traductions
 * doivent donc être imbriquées, pas aplaties en `"pi.autoHide"`.
 */
function flatten(source: Record<string, unknown>, prefix = ""): Map<string, unknown> {
  const entries = new Map<string, unknown>();
  for (const [key, value] of Object.entries(source)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      for (const [nested, leaf] of flatten(value as Record<string, unknown>, path)) {
        entries.set(nested, leaf);
      }
    } else {
      entries.set(path, value);
    }
  }
  return entries;
}

const localization = (locale: Record<string, unknown>) =>
  flatten(locale.Localization as Record<string, unknown>);

test("les deux langues couvrent exactement les memes cles", () => {
  assert.deepEqual([...localization(en).keys()].sort(), [...localization(fr).keys()].sort());
});

test("aucune cle de traduction n'est aplatie avec un point", () => {
  for (const [language, locale] of [
    ["en", en],
    ["fr", fr],
  ] as const) {
    for (const group of Object.keys(locale.Localization as Record<string, unknown>)) {
      assert.ok(
        !group.includes("."),
        `${group} doit etre imbrique dans ${language}.json, sinon le SDK ne le trouve pas`,
      );
    }
  }
});

test("chaque action du manifest est traduite dans les deux langues", () => {
  for (const { UUID } of manifest.Actions) {
    for (const [language, locale] of [
      ["en", en],
      ["fr", fr],
    ] as const) {
      const entry = locale[UUID] as { Name?: string; Tooltip?: string } | undefined;
      assert.ok(entry, `${UUID} absent de ${language}.json`);
      assert.ok(entry.Name, `${UUID} sans Name dans ${language}.json`);
      assert.ok(entry.Tooltip, `${UUID} sans Tooltip dans ${language}.json`);
    }
  }
});

test("aucune traduction n'est vide", () => {
  for (const [language, locale] of [
    ["en", en],
    ["fr", fr],
  ] as const) {
    for (const [key, value] of localization(locale)) {
      assert.equal(typeof value, "string", `${key} n'est pas une chaine dans ${language}.json`);
      assert.ok(
        (value as string).trim().length > 0,
        `${key} est vide dans ${language}.json`,
      );
    }
  }
});
