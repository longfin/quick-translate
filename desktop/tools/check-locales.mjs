#!/usr/bin/env node
// Checks the UI string catalogs in src/locales/{en,ko,ja}.json. Keys are the English strings and
// en.json lists every key; ko.json and ja.json must have exactly the same keys, and a value must
// keep its key's printf-style placeholders (%@, %d, %.2f, ...). Lists the problems and exits 1.
//
// Usage: node tools/check-locales.mjs   (or `pnpm strings:check`; `pnpm build` runs it too)

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const LOCALES = ["en", "ko", "ja"];
const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "locales");
const load = (locale) => JSON.parse(readFileSync(join(dir, `${locale}.json`), "utf8"));
const placeholders = (s) => (s.match(/%(?:\d+\$)?[-+ #0]*\d*(?:\.\d+)?(?:@|l?[dfsu])/g) ?? []).sort().join(" ");

const catalogs = Object.fromEntries(LOCALES.map((l) => [l, load(l)]));
const problems = [];
for (const locale of LOCALES) {
  const catalog = catalogs[locale];
  for (const key of Object.keys(catalogs.en)) {
    if (!(key in catalog)) problems.push(`${locale}: missing ${JSON.stringify(key)}`);
    else if (placeholders(catalog[key]) !== placeholders(key))
      problems.push(`${locale}: placeholders differ in ${JSON.stringify(key)}`);
  }
  for (const key of Object.keys(catalog)) {
    if (!(key in catalogs.en)) problems.push(`${locale}: ${JSON.stringify(key)} is not in en.json`);
  }
}
if (problems.length > 0) {
  console.error(problems.join("\n"));
  process.exit(1);
}
console.log(`src/locales: ${Object.keys(catalogs.en).length} strings in ${LOCALES.join(", ")}`);
