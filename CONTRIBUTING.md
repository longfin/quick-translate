# Contributing

Thanks for taking a look. QuickTranslate is two apps sharing a design and the UI strings: the macOS
app in `macos/`, a small Swift Package Manager project with no Xcode project file, and the Windows /
Linux app in `desktop/`, built with Tauri 2. Both work from the command line.

## Development loop (macOS)

```bash
cd macos
./make-cert.sh          # once: stable signing identity so the Accessibility grant survives rebuilds
./build.sh --install    # build, install to /Applications, relaunch
tail -f ~/Library/Logs/QuickTranslate.log
```

- `swift build` alone produces a bare executable; the app must run as the `.app` bundle (menu bar
  behaviour, localization, Accessibility) so always go through `build.sh`.
- The log file records permission state, hotkey detection, detected/target languages and CLI exit
  codes. Most bug reports can be answered from it.
- To trigger a translation without the hotkey (e.g. from a script):
  `osascript -l JavaScript -e 'ObjC.import("Foundation"); $.NSDistributedNotificationCenter.defaultCenter.postNotificationNameObject("dev.swen.QuickTranslate.translate", $())'`

## Development loop (Windows / Linux)

Prerequisites (Rust, Node 20+, pnpm, and the Linux packages) are in
[`desktop/README.md`](desktop/README.md). The app also builds and runs on a Mac for development.

```bash
cd desktop
pnpm install
pnpm tauri dev                     # Vite dev server + debug build; Rust changes rebuild and relaunch
cd src-tauri && cargo clippy --all-targets && cargo test
```

- There is no window at startup: look for the tray icon. Logs (also echoed to the `pnpm tauri dev`
  terminal): `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` on Windows,
  `~/.local/state/QuickTranslate/QuickTranslate.log` on Linux, `~/Library/Logs/QuickTranslate-desktop.log`
  for dev builds on macOS.
- To trigger without the keyboard, run the binary again while it is running: `quicktranslate
  --translate` translates the clipboard and `quicktranslate --settings` opens Settings
  (`src-tauri/target/debug/quicktranslate` in a dev build).
- **Linux smoke test**, headless: `desktop/scripts/linux-smoke.sh [binary]` starts Xvfb and a private
  D-Bus session, runs a release build (default `src-tauri/target/release/quicktranslate`), copies twice
  with `xclip`, sends `--translate` and `--settings`, and checks the log lines and the mapped windows.
  It needs `xvfb xclip dbus-x11 xdotool` and no `claude` CLI; CI runs it after every Linux build.
  For a real GNOME Shell (tray menu, Wayland), see [`desktop/scripts/gnome-headless`](desktop/scripts/gnome-headless).
- UI strings come from the macOS `.strings` files; after editing them run `pnpm strings:json` in
  `desktop/` and commit the generated JSON.

## Layout

See "Project layout" in the README and [`desktop/README.md`](desktop/README.md). Rough rules:

- macOS: AppKit for the window/menu/hotkey plumbing, SwiftUI for the views.
- **The macOS app has no third-party dependencies.** Both apps must keep working without network
  access to anything other than what the `claude` / `codex` CLIs do themselves.
- `desktop/` can't avoid dependencies, but keeps them few. Rule of thumb: official Tauri plugins
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
  | `objc2-app-kit` | the dev-only clipboard watcher on macOS |

  Adding to it is a conscious decision: say why in the PR, and update this table.
- No API keys, tokens or telemetry in either app. Authentication is the CLI's job.

## Adding a UI language

1. Create `macos/Resources/<code>.lproj/Localizable.strings` (copy `ko.lproj` as a starting point).
   Keys are the English strings; anything missing falls back to English.
2. Add the language to `AppSettings.uiLanguages` in `macos/Sources/QuickTranslate/Settings.swift`.
3. Run `plutil -lint macos/Resources/*/Localizable.strings`.

## Adding a translation language

Add the English name to `AppSettings.languages`, its localized names to each `.strings` file under
`macos/Resources/`, the `NLLanguage` mapping in `LanguageDetector.swift`, and the locale code mapping in
`AppSettings.systemDefaultLanguages()`.

## Pull requests

- Keep PRs focused; one change per PR.
- Describe how you tested it (which CLI, which OS version, X11 or Wayland) and paste relevant log
  lines.
- macOS: run `swift build -c release` in `macos/` before pushing; there is no CI for it yet.
- `desktop/`: CI ([`desktop.yml`](.github/workflows/desktop.yml)) builds Windows x64 and Linux
  x86_64 on every pull request and runs the Linux smoke test. Running it by hand (Actions → desktop →
  Run workflow) also builds Windows ARM64 and Linux aarch64.

## Releasing the desktop app

1. Set the version in `desktop/package.json`, `desktop/src-tauri/tauri.conf.json` and
   `desktop/src-tauri/Cargo.toml` (then `cargo check` to update `Cargo.lock`), and move the
   "Unreleased" notes in `desktop/CHANGELOG.md` under that version.
2. Push a tag `desktop-v<version>`. [`release.yml`](.github/workflows/release.yml) checks the tag
   against the version, builds all four platforms and creates a **draft** release with the bundles
   and `SHA256SUMS`.
3. Try the bundles, edit the notes, publish the draft.

The macOS app is not part of this release.

## Reporting bugs

Include your OS and version, which engine (Claude / Codex) and model, the output of
`claude --version` or `codex --version`, and the last ~20 lines of the log:
`~/Library/Logs/QuickTranslate.log` (macOS), `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log`
(Windows) or `~/.local/state/QuickTranslate/QuickTranslate.log` (Linux).
