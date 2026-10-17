#!/usr/bin/env node
// Converts the macOS app's Localizable.strings into the desktop UI's JSON catalogs:
//   macos/Resources/{en,ko,ja}.lproj/Localizable.strings -> desktop/src/locales/{en,ko,ja}.json
//
// Keys are the English strings. en.lproj only lists strings whose English text differs from the
// key, so en.json holds every key found in any locale (value = en override, else the key). A key
// missing from ko or ja fails the run unless --allow-missing is given.
//
// Usage: node tools/strings-to-json.mjs [--allow-missing]   (or `pnpm strings:json`)

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const LOCALES = ["en", "ko", "ja"];
const desktop = join(dirname(fileURLToPath(import.meta.url)), "..");
const resources = join(desktop, "..", "macos", "Resources");
const outDir = join(desktop, "src", "locales");
const allowMissing = process.argv.includes("--allow-missing");

/** Parses `"key" = "value";` pairs, skipping `//` and `/* *\/` comments. */
function parseStrings(text, file) {
  const entries = new Map();
  let i = 0;
  const fail = (msg) => {
    const line = text.slice(0, i).split("\n").length;
    throw new Error(`${file}:${line}: ${msg}`);
  };
  const skip = () => {
    for (;;) {
      if (/\s/.test(text[i] ?? "")) i++;
      else if (text.startsWith("//", i)) {
        const end = text.indexOf("\n", i);
        i = end === -1 ? text.length : end;
      } else if (text.startsWith("/*", i)) {
        const end = text.indexOf("*/", i + 2);
        if (end === -1) fail("unterminated comment");
        i = end + 2;
      } else return;
    }
  };
  const expect = (ch) => {
    skip();
    if (text[i] !== ch) fail(`expected '${ch}', found ${JSON.stringify(text[i] ?? "end of file")}`);
    i++;
  };
  const string = () => {
    expect('"');
    let out = "";
    for (;;) {
      const ch = text[i++];
      if (ch === undefined) fail("unterminated string");
      if (ch === '"') return out;
      if (ch !== "\\") {
        out += ch;
        continue;
      }
      const esc = text[i++];
      if (esc === "n") out += "\n";
      else if (esc === "t") out += "\t";
      else if (esc === "r") out += "\r";
      else if (esc === '"' || esc === "\\" || esc === "'") out += esc;
      else if (esc === "U" || esc === "u") {
        const hex = text.slice(i, i + 4);
        if (!/^[0-9a-fA-F]{4}$/.test(hex)) fail(`bad \\${esc} escape`);
        out += String.fromCharCode(parseInt(hex, 16));
        i += 4;
      } else fail(`unknown escape \\${esc}`);
    }
  };
  for (;;) {
    skip();
    if (i >= text.length) return entries;
    const key = string();
    expect("=");
    const value = string();
    expect(";");
    if (entries.has(key)) fail(`duplicate key ${JSON.stringify(key)}`);
    entries.set(key, value);
  }
}

const parsed = {};
for (const locale of LOCALES) {
  const file = join(resources, `${locale}.lproj`, "Localizable.strings");
  try {
    parsed[locale] = parseStrings(readFileSync(file, "utf8").replace(/^﻿/, ""), relative(desktop, file));
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}

// English: every known key, with en.lproj's text where it overrides the key.
const allKeys = new Set(LOCALES.flatMap((l) => [...parsed[l].keys()]));
for (const key of allKeys) {
  if (!parsed.en.has(key)) parsed.en.set(key, key);
}

let missingCount = 0;
for (const locale of LOCALES.filter((l) => l !== "en")) {
  const missing = [...parsed.en.keys()].filter((k) => !parsed[locale].has(k)).sort();
  if (missing.length === 0) continue;
  missingCount += missing.length;
  console.error(`${locale}: ${missing.length} key(s) missing:`);
  for (const key of missing) console.error(`  ${JSON.stringify(key)}`);
}
if (missingCount > 0 && !allowMissing) {
  console.error("Add the translations, or pass --allow-missing to fall back to English.");
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });
for (const locale of LOCALES) {
  const sorted = Object.fromEntries([...parsed[locale]].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  const out = join(outDir, `${locale}.json`);
  writeFileSync(out, JSON.stringify(sorted, null, 2) + "\n");
  console.log(`${relative(desktop, out)}: ${parsed[locale].size} strings`);
}
