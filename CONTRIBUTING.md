# Contributing

Thanks for taking a look. QuickTranslate is one Tauri 2 app in `desktop/` for macOS, Windows and
Linux: Rust in `src-tauri/`, a vanilla TypeScript web UI in `src/`. It works from the command line.

## Development loop

Prerequisites (Rust, Node 20+, pnpm, and the Linux packages) are in
[`desktop/README.md`](desktop/README.md).

```bash
cd desktop
pnpm install
pnpm tauri dev                     # Vite dev server + debug build; Rust changes rebuild and relaunch
cd src-tauri && cargo clippy --all-targets && cargo test
```

- There is no window at startup: look for the tray icon. Logs (also echoed to the `pnpm tauri dev`
  terminal): `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` on Windows,
  `~/.local/state/QuickTranslate/QuickTranslate.log` on Linux, `~/Library/Logs/QuickTranslate.log` on
  macOS. They record the clipboard watcher in use, triggers, detected/target languages and CLI exit
  codes; most bug reports can be answered from them.
- To trigger without the keyboard, run the binary again while it is running: `quicktranslate
  --translate` translates the clipboard and `quicktranslate --settings` opens Settings
  (`src-tauri/target/debug/quicktranslate` in a dev build).
- **Linux smoke test**, headless: `desktop/scripts/linux-smoke.sh [binary]` starts Xvfb and a private
  D-Bus session, runs a release build (default `src-tauri/target/release/quicktranslate`), copies twice
  with `xclip`, sends `--translate` and `--settings`, and checks the log lines and the mapped windows.
  It needs `xvfb xclip dbus-x11 xdotool` and no `claude` CLI; CI runs it after every Linux build.
  For a real GNOME Shell (tray menu, Wayland), see [`desktop/scripts/gnome-headless`](desktop/scripts/gnome-headless).
- UI strings live in `desktop/src/locales/{en,ko,ja}.json`; `pnpm strings:check` (also part of
  `pnpm build`) fails when a locale lacks a key or a placeholder.

## Layout

See "Project layout" in the README and [`desktop/README.md`](desktop/README.md). Rough rules:

- The app must keep working without network access to anything other than what the `claude` /
  `codex` CLIs do themselves.
- It can't avoid dependencies, but keeps them few. Rule of thumb: official Tauri plugins
  are fine; anything else must be small, widely used, and do something the platform API can't
  reasonably do in a few dozen lines. The web UI is vanilla TypeScript (no framework). Current
  allowlist, from `desktop/src-tauri/Cargo.toml`:

  | Crate | Why |
  | --- | --- |
  | `tauri`, `tauri-build` | the app framework (with `tray-icon`) |
  | `tauri-plugin-single-instance` | one instance; `--translate` / `--settings` reach the running one |
  | `tauri-plugin-global-shortcut` | the single-press hotkey |
  | `tauri-plugin-autostart` | "Run at startup" (Run key / XDG autostart) |
  | `tokio` | child processes with streamed output (already in Tauri's tree) |
  | `serde`, `serde_json` | settings file, CLI stream-json, IPC payloads |
  | `chrono` | log timestamps (clock only) |
  | `arboard` | reading the clipboard text on all three platforms |
  | `sys-locale` | the system languages, for the default target language |
  | `whatlang` | language detection for Latin-script text (no default features) |
  | `libc` | `setsid` / `killpg` to cancel a CLI's whole process tree (Unix) |
  | `windows-sys` | Job Objects and the clipboard listener (Windows; already in Tauri's tree) |
  | `x11rb` | XFixes clipboard notifications (Linux X11 / XWayland) |
  | `wayland-client`, `wayland-protocols`, `wayland-protocols-wlr` | data-control clipboard notifications (Linux Wayland) |
  | `objc2-app-kit` | the clipboard watcher on macOS |
  | `tauri-nspanel` | the non-activating `NSPanel` for the translation window (macOS only) |

  Adding to it is a conscious decision: say why in the PR, and update this table.
- No API keys, tokens or telemetry. Authentication is the CLI's job.

## Adding a UI language

1. Copy `desktop/src/locales/ko.json` to `<code>.json` and translate the values. Keys are the
   English strings; keep placeholders such as `%@` and `%d`.
2. Register it in `desktop/src/i18n.ts` (`catalogs`, `LOCALES`, `systemLocale`),
   `desktop/src-tauri/src/i18n.rs` and `UI_LANGUAGES` in `desktop/src-tauri/src/engine/settings.rs`,
   and add it to `LOCALES` in `desktop/tools/check-locales.mjs`.
3. Run `pnpm strings:check`.

## Adding a translation language

Add it to `LANGUAGES` in `desktop/src-tauri/src/engine/language.rs` (English name, locale code)
with its detection rule, and its localized names to each `desktop/src/locales/*.json`.

## Pull requests

- Keep PRs focused; one change per PR.
- Describe how you tested it (which CLI, which OS version, X11 or Wayland) and paste relevant log
  lines.
- CI ([`desktop.yml`](.github/workflows/desktop.yml)) builds Windows x64, Linux x86_64 and macOS
  (universal) on every pull request and runs the Linux smoke test. Running it by hand (Actions →
  desktop → Run workflow) also builds Windows ARM64 and Linux aarch64.

## Releasing

1. Set the version in `desktop/package.json`, `desktop/src-tauri/tauri.conf.json` and
   `desktop/src-tauri/Cargo.toml` (then `cargo check` to update `Cargo.lock`), and move the
   "Unreleased" notes in `desktop/CHANGELOG.md` under that version.
2. Push a tag `desktop-v<version>`. [`release.yml`](.github/workflows/release.yml) checks the tag
   against the version, builds all five platforms and creates a **draft** release with the bundles
   and `SHA256SUMS`.
3. Try the bundles, edit the notes, publish the draft.

## Reporting bugs

Include your OS and version, which engine (Claude / Codex) and model, the output of
`claude --version` or `codex --version`, and the last ~20 lines of the log:
`~/Library/Logs/QuickTranslate.log` (macOS), `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log`
(Windows) or `~/.local/state/QuickTranslate/QuickTranslate.log` (Linux).
