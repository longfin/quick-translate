# QuickTranslate (desktop/)

The QuickTranslate app for macOS, Windows and Linux, built with Tauri 2. It replaced the native
Swift macOS app ([#20](https://github.com/longfin/quick-translate/issues/20)); the original plan
for Windows and Linux is [#3](https://github.com/longfin/quick-translate/issues/3).

User-facing docs (requirements, install, Wayland, troubleshooting) are in the root
[README](../README.md); changes in [CHANGELOG.md](CHANGELOG.md).

## Layout

```
src/            web UI (vanilla TypeScript + Vite): main.ts = translation panel (index.html),
                settings.ts = Settings window (settings.html), theme.css shared; i18n.ts = t(key)
src-tauri/
  src/lib.rs    app setup, single instance
  src/settings.rs  settings.json in memory, applying changes, the Settings window (Phase 4)
  src/i18n.rs   the same string catalogs for the tray menu
  src/tray.rs   tray icon + menu
  src/log.rs    log file
  src/engine/   claude / codex CLI engine (Phase 1)
  src/trigger/  Ctrl+C Ctrl+C, hotkey (Phase 2)
  src/panel.rs  translation panel: placement near the cursor, pin, languages (Phase 3)
  src/macos.rs  the translation window as a non-activating NSPanel (macOS)
  Info.plist    LSUIElement (menu bar only), merged into the macOS bundle
  icons/        app icons (full colour) and tray.svg -> tray-{32,64}.png (monochrome tray glyph)
scripts/
  linux-smoke.sh       headless smoke test (Xvfb), run in CI
  build-tray-icons.sh  renders icons/tray.svg
  gnome-headless/      headless GNOME Shell sessions for checking the tray by hand
tools/          check-locales (src/locales/*.json have the same keys and placeholders)
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
- **macOS**: Xcode Command Line Tools; for the universal bundle also
  `rustup target add x86_64-apple-darwin aarch64-apple-darwin`.

See also Tauri's [prerequisites](https://v2.tauri.app/start/prerequisites/).

## Dev loop

```bash
cd desktop
pnpm install
pnpm tauri dev      # run with the Vite dev server; Rust changes rebuild automatically
pnpm tauri build    # release build + bundles in src-tauri/target/release/bundle/
```

Bundles: NSIS installer on Windows, AppImage and `.deb` on Linux, `.app` on macOS (`--bundles nsis`
etc. to pick one; `--target universal-apple-darwin` for an arm64 + x86_64 `.app`). CI is described under [Release](#release).

The app has no window at startup and no taskbar entry; look for the tray icon. Launching it a
second time does not start another copy: the running instance logs the second launch's arguments.

## Triggers

Copying twice within the double-press interval (Ctrl+C Ctrl+C, 400 ms by default) or, if chosen
in Settings instead, a single-press hotkey (`Ctrl+Shift+T` by default), the tray's "Translate
clipboard" and `quicktranslate --translate` (sent to the running instance) all end in
`on_trigger` in `lib.rs`, which shows the panel near the cursor (`panel::show_for`). The log names the clipboard watcher in use (`trigger: clipboard watcher =
…`): Windows clipboard listener, X11 XFixes, Wayland data-control (wlroots, KDE), XWayland, or
`Unsupported` (e.g. GNOME on Wayland without XWayland: use the tray item or the hotkey).

The hotkey translates what is already on the clipboard; unlike the old Swift app it doesn't copy
the selection first yet.

To regenerate the icons from `docs/icon-1024.png`, run `pnpm tauri icon ../docs/icon-1024.png`,
then delete the generated `android/`, `ios/`, `Square*Logo.png` and `StoreLogo.png`, which aren't
used. The tray uses its own monochrome glyph, `src-tauri/icons/tray.svg` (the macOS menu bar icon in
white); after editing it run `scripts/build-tray-icons.sh` (needs `rsvg-convert`). The PNGs are
decoded at compile time (`tauri::include_image!`). On Windows the glyph follows the system's
light / dark mode (black on the default light taskbar, white in dark mode; read at startup). On
Linux it stays white, which suits most panels; on a light Linux panel it is hard to see, since there
is no portable way to ask a panel for its colour.

## Panel

Esc or Ctrl+W (Cmd+W on macOS) hides it and cancels the translation; Ctrl+Enter retranslates.
Unpinned, it hides when it loses focus ("close when clicking outside"), except while a
translation is still streaming, like the Swift app did. Pinned, it stays on top and stays open. A
trigger while the panel is visible replaces the text and restarts the translation in place.

The target language comes from `targetLanguage` / `fallbackLanguage` in the settings when set,
else from the system languages (same rule as the Swift app); text already in the target
language goes to the fallback.

## Settings

The tray's "Settings…" or `quicktranslate --settings` (also to a running instance) opens the
Settings window: engine (Claude / Codex, model, CLI path with **Test**), target / fallback /
interface language, trigger (Ctrl+C twice with its interval, or a single-press hotkey recorded
from the keyboard), close when clicking outside, keep Claude warm and run at startup. Every change
is saved and applied at once; Esc or Ctrl+W closes the window.

They are stored as JSON in `<config dir>/dev.longfin.quicktranslate/settings.json`
(`%APPDATA%\dev.longfin.quicktranslate\settings.json` on Windows,
`~/.config/dev.longfin.quicktranslate/settings.json` on Linux,
`~/Library/Application Support/dev.longfin.quicktranslate/settings.json` on macOS), with the Swift
app's `UserDefaults` keys:

| Key | Default | |
| --- | --- | --- |
| `backend` | `"claude"` | or `"codex"` |
| `claudeModel` / `codexModel` | `"haiku"` / `""` | blank = the CLI's default |
| `claudePath` / `codexPath` | `""` | blank = auto-detect |
| `keepClaudeWarm` | `true` | resident `claude` process |
| `targetLanguage` / `fallbackLanguage` | unset | unset = from the system languages |
| `uiLanguage` | `"system"` | `"en"`, `"ko"`, `"ja"`; applies without a restart |
| `hotkeyDoublePress` | `true` | `true` = Ctrl+C Ctrl+C, `false` = the `hotkey` |
| `hotkey` | `"Ctrl+Shift+T"` | desktop only (the Swift app stored macOS key codes) |
| `doublePressInterval` | `0.4` | seconds, 0.2–1.0 |
| `closeOnOutsideClick` | `true` | |
| `launchAtLogin` | `false` | desktop only; the HKCU `Run` key / `~/.config/autostart/QuickTranslate.desktop` / the LaunchAgent wins when they differ |

A value of the wrong type falls back to its default; unknown keys are kept. The running app
also notices when the file is edited by hand (within a second) and applies it.

## Localization

UI strings live in [`src/locales/`](src/locales) (English, Korean, Japanese), keyed by the English
text; en.json lists every key. Strings were carried over from the Swift app's `Localizable.strings`,
so some (Accessibility, Secure Keyboard Entry) are not used by this app. After editing them, run

```bash
pnpm strings:check   # also part of `pnpm build`
```

It fails, listing the keys, when Korean or Japanese lacks a string or a placeholder (`%@`, `%d`, …)
differs. In the UI, `t("English text", { name: value })` from `src/i18n.ts` looks a string up.

## macOS

- Menu-bar-only app (`LSUIElement`, `ActivationPolicy::Accessory`), template tray glyph.
- The translation window is a non-activating `NSPanel` ([tauri-nspanel](https://crates.io/crates/tauri-nspanel)):
  showing it keeps the frontmost app's focus and menu bar; Esc / Cmd+W and "close when clicking
  outside" still work because the panel becomes key. It floats on every Space.
- Cmd+C Cmd+C through a 60 ms `NSPasteboard.changeCount` poll (the pasteboard has no change
  notification); the hotkey through the system hot key API. No Accessibility permission.
- The bundle is universal (arm64 + x86_64) and ad-hoc signed (`signingIdentity: "-"`), minimum
  macOS 13. A downloaded copy needs `xattr -d com.apple.quarantine` until there is a Developer ID
  build.
- Bundle id `dev.longfin.quicktranslate` (the Swift app was `dev.swen.QuickTranslate`).
- Not reproduced from the Swift app: the native title bar / toolbar chrome (#2). Memory sits around
  4× the Swift app's idle footprint (WebKit helper processes).

## Cross-building for Windows from macOS

A plain `.exe` (no installer) can be built on an Apple Silicon Mac with
[cargo-xwin](https://github.com/rust-cross/cargo-xwin), which downloads the Windows SDK and CRT
from Microsoft on first use (about 1 GB, into `~/Library/Caches/cargo-xwin`; using them means
accepting the Visual Studio license):

```bash
brew install llvm lld                       # clang-cl, llvm-rc, lld-link
cargo install --locked cargo-xwin
rustup target add aarch64-pc-windows-msvc   # or x86_64-pc-windows-msvc

cd desktop && pnpm install && pnpm build    # web UI into dist/
cd src-tauri
PATH=/opt/homebrew/opt/llvm/bin:$PATH cargo xwin build --release \
  --target aarch64-pc-windows-msvc --features tauri/custom-protocol
# → target/aarch64-pc-windows-msvc/release/quicktranslate.exe
```

`--features tauri/custom-protocol` matters: `tauri build` adds it for you, but a plain `cargo build`
without it is a *dev* build that loads the UI from the Vite dev server (`devUrl`), so the panel shows
WebView2's "can't reach this page" instead of the app.

The result is self-contained: the WebView2 loader and the VC runtime are linked statically, so it
runs as-is on Windows 11 (the WebView2 Runtime ships with the OS). Builds take about a minute
after the first SDK download. NSIS/MSI installers still need Windows; CI produces those.

## Logs

Timestamped lines, appended, in the same format as the Swift app:

| OS | Path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` |
| Linux | `$XDG_STATE_HOME/QuickTranslate/QuickTranslate.log` (default `~/.local/state/QuickTranslate/QuickTranslate.log`) |
| macOS | `~/Library/Logs/QuickTranslate.log` |

The same lines also go to stderr, so they show up in the `pnpm tauri dev` terminal.

## Release

| | Runner | Bundles |
| --- | --- | --- |
| Windows x64 | `windows-latest` | `QuickTranslate-<v>-windows-x64-setup.exe`, `…-windows-x64-portable.zip` |
| Windows ARM64 | `windows-11-arm` | `QuickTranslate-<v>-windows-arm64-setup.exe`, `…-windows-arm64-portable.zip` |
| Linux x86_64 | `ubuntu-22.04` | `QuickTranslate-<v>-linux-x86_64.AppImage`, `QuickTranslate_<v>_amd64.deb` |
| Linux aarch64 | `ubuntu-22.04-arm` | `QuickTranslate-<v>-linux-aarch64.AppImage`, `QuickTranslate_<v>_arm64.deb` |
| macOS universal | `macos-latest` | `QuickTranslate-<v>-macos-universal.app.zip` (zipped `.app`, `lipo` and `codesign --verify` checked) |

CI builds are native (no cross-compiling; for a local Windows exe from a Mac see
[Cross-building](#cross-building-for-windows-from-macos)). Linux builds on Ubuntu 22.04 so the bundles run on
**glibc 2.35** and newer. The portable zip is `quicktranslate.exe` plus `LICENSE.txt`. The `.deb`
package is called `quick-translate` and depends on `libayatana-appindicator3-1`,
`libwebkit2gtk-4.1-0` and `libgtk-3-0`.

- [`desktop-build.yml`](../.github/workflows/desktop-build.yml) (reusable) builds the matrix, runs
  [`scripts/linux-smoke.sh`](scripts/linux-smoke.sh) on both Linux runners and uploads one artifact
  per platform with the names above.
- [`desktop.yml`](../.github/workflows/desktop.yml) runs it for pushes to `main` and pull requests
  (Windows x64, Linux x86_64 and macOS, to stay fast) and for "Run workflow" (all five).
- [`release.yml`](../.github/workflows/release.yml) runs on a `desktop-v<version>` tag: checks that
  the tag matches the version in `package.json`, `tauri.conf.json` and `Cargo.toml`, builds all five,
  and creates a **draft** GitHub release with the bundles, `SHA256SUMS` and the notes from
  [`.github/desktop-release-notes.md`](../.github/desktop-release-notes.md). Steps in
  [CONTRIBUTING](../CONTRIBUTING.md#releasing).

The builds are **not code-signed**: Windows SmartScreen asks for "More info → Run anyway", macOS
needs the quarantine flag cleared, and the release notes say so. Signing, winget and Flathub are for later.
