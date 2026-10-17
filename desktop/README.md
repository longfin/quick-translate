# QuickTranslate for Windows and Linux

A Tauri 2 tray app that brings QuickTranslate to Windows and Linux. The macOS app in
[`../macos`](../macos) is still the native Swift one; both apps share the same design but no code.
The plan and its progress are tracked in [#3](https://github.com/longfin/quick-translate/issues/3).

**Status: Phase 3 (translation panel).** Tray, triggers, engine and the translation panel work;
the tray's "Settings…" only writes a log line for now (Phase 4).

## Layout

```
src/            web UI (vanilla TypeScript + Vite): the translation panel; i18n.ts = t(key)
src-tauri/
  src/lib.rs    app setup, single instance
  src/tray.rs   tray icon + menu
  src/log.rs    log file
  src/engine/   claude / codex CLI engine (Phase 1)
  src/trigger/  Ctrl+C Ctrl+C, hotkey (Phase 2)
  src/panel.rs  translation panel: placement near the cursor, pin, languages (Phase 3)
```

## Prerequisites

- [Rust](https://rustup.rs) (stable), Node.js 20+, [pnpm](https://pnpm.io) 10
- **Windows**: Microsoft C++ Build Tools ("Desktop development with C++") and WebView2. WebView2
  ships with Windows 10 (since 1803) and 11.
- **Linux** (Debian/Ubuntu):
  ```bash
  sudo apt install build-essential file patchelf libssl-dev libgtk-3-dev \
    libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev
  ```
  On GNOME, the tray icon needs the AppIndicator extension.
- **macOS**: Xcode Command Line Tools. macOS isn't a release target, but the app builds and runs
  there for development.

See also Tauri's [prerequisites](https://v2.tauri.app/start/prerequisites/).

## Dev loop

```bash
cd desktop
pnpm install
pnpm tauri dev      # run with the Vite dev server; Rust changes rebuild automatically
pnpm tauri build    # release build + bundles in src-tauri/target/release/bundle/
```

Bundles: NSIS installer and MSI on Windows, AppImage and `.deb` on Linux. CI
([`desktop.yml`](../.github/workflows/desktop.yml)) builds both and uploads them as artifacts.

The app has no window at startup and no taskbar entry; look for the tray icon. Launching it a
second time does not start another copy: the running instance logs the second launch's arguments.

## Triggers

Copying twice within 400 ms (Ctrl+C Ctrl+C), the tray's "Translate clipboard", the optional
`Ctrl+Shift+T` hotkey and `quicktranslate --translate` (sent to the running instance) all end in
`on_trigger` in `lib.rs`, which shows the panel near the cursor (`panel::show_for`). The log names the clipboard watcher in use (`trigger: clipboard watcher =
…`): Windows clipboard listener, X11 XFixes, Wayland data-control (wlroots, KDE), XWayland, or
`Unsupported` (e.g. GNOME on Wayland without XWayland: use the tray item or the hotkey).

Until Settings exist, two environment variables help during development:
`QUICKTRANSLATE_INTERVAL_MS=<ms>` changes the double-copy interval and `QUICKTRANSLATE_HOTKEY=1`
turns the hotkey on.

To regenerate the icons from `docs/icon-1024.png`, run `pnpm tauri icon ../docs/icon-1024.png`,
then delete the generated `android/`, `ios/`, `Square*Logo.png` and `StoreLogo.png`, which aren't
used.

## Panel

Esc or Ctrl+W (Cmd+W on macOS) hides it and cancels the translation; Ctrl+Enter retranslates.
Unpinned, it hides when it loses focus ("close when clicking outside"), except while a
translation is still streaming, like the macOS app. Pinned, it stays on top and stays open. A
trigger while the panel is visible replaces the text and restarts the translation in place.

The target language comes from `targetLanguage` / `fallbackLanguage` in the settings file when
set, else from the system languages (same rule as the macOS app); text already in the target
language goes to the fallback.

## Localization

UI strings are shared with the macOS app: they live in
[`macos/Resources/*.lproj/Localizable.strings`](../macos/Resources) (English, Korean, Japanese),
keyed by the English text. Strings only the desktop app uses sit in their own section at the end of
each file. After editing them, run

```bash
pnpm strings:json   # regenerates src/locales/{en,ko,ja}.json; commit the result
```

It fails, listing the keys, when Korean or Japanese lacks a string (`--allow-missing` falls back to
English). In the UI, `t("English text", { name: value })` from `src/i18n.ts` looks a string up.

## Logs

Timestamped lines, appended, in the same format as the macOS app:

| OS | Path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` |
| Linux | `$XDG_STATE_HOME/QuickTranslate/QuickTranslate.log` (default `~/.local/state/QuickTranslate/QuickTranslate.log`) |
| macOS (dev only) | `~/Library/Logs/QuickTranslate-desktop.log` |

The same lines also go to stderr, so they show up in the `pnpm tauri dev` terminal.
