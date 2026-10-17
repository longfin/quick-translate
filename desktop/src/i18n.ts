// UI strings. Keys are the English strings (like the Swift app's Localizable.strings), so a
// missing translation falls back to English. Phase 6 replaces this file with the locale loader;
// keep `t` the only export the UI uses.
export function t(key: string): string {
  return key;
}
