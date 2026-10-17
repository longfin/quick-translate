// UI strings. The catalogs are generated from the macOS app's Localizable.strings
// (`pnpm strings:json`); keys are the English strings, as in the Swift app.
import en from "./locales/en.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";

export type Locale = "en" | "ko" | "ja";
export type LocalePreference = "system" | Locale;

type Catalog = Record<string, string>;
const catalogs: Record<Locale, Catalog> = { en, ko, ja };

const LOCALES: readonly string[] = ["en", "ko", "ja"];
let locale: Locale = "en";

const lookup = (catalog: Catalog, key: string): string | undefined =>
  Object.prototype.hasOwnProperty.call(catalog, key) ? catalog[key] : undefined;

/** Maps a BCP 47 tag (`navigator.language`) to a UI locale: ko*, ja*, else en. */
export function systemLocale(tag: string = navigator.language): Locale {
  const lower = tag.toLowerCase();
  if (lower.startsWith("ko")) return "ko";
  if (lower.startsWith("ja")) return "ja";
  return "en";
}

/** Picks the UI locale from the `uiLanguage` setting ("system", "en", "ko" or "ja"). */
export function initI18n(preferred: LocalePreference = "system"): Locale {
  setLocale(preferred === "system" ? systemLocale() : preferred);
  return locale;
}

export function setLocale(code: string): void {
  locale = LOCALES.includes(code) ? (code as Locale) : "en";
  document.documentElement.lang = locale;
}

export function currentLocale(): Locale {
  return locale;
}

/**
 * Translates `key` (the English string), falling back to English, then to the key itself.
 * `{name}` placeholders are replaced from `vars`; unknown placeholders are left as they are.
 */
export function t(key: string, vars?: Record<string, string | number>): string {
  const text = lookup(catalogs[locale], key) ?? lookup(catalogs.en, key) ?? key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : match,
  );
}
