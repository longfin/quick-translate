# QuickTranslate for Windows and Linux

A Tauri 2 tray app that brings QuickTranslate to Windows and Linux. The macOS app in
[`../macos`](../macos) is still the native Swift one; both apps share the same design but no code.
The plan and its progress are tracked in [#3](https://github.com/longfin/quick-translate/issues/3).

**Status: Phase 0 (scaffold).** The tray icon, its menu and single-instance handling work. The
menu's "Translate clipboard" and "Settings…" items only write a log line for now.

## Layout

```
src/            web UI (vanilla TypeScript + Vite)
src-tauri/
  src/lib.rs    app setup, single instance
  src/tray.rs   tray icon + menu
  src/log.rs    log file
  src/engine/   claude / codex CLI engine (Phase 1)
  src/trigger/  Ctrl+C Ctrl+C, hotkey (Phase 2)
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

To regenerate the icons from `docs/icon-1024.png`, run `pnpm tauri icon ../docs/icon-1024.png`,
then delete the generated `android/`, `ios/`, `Square*Logo.png` and `StoreLogo.png`, which aren't
used.

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

Timestamped lines, appended, in the same format as the macOS app:

| OS | Path |
| --- | --- |
| Windows | `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` |
| Linux | `$XDG_STATE_HOME/QuickTranslate/QuickTranslate.log` (default `~/.local/state/QuickTranslate/QuickTranslate.log`) |
| macOS (dev only) | `~/Library/Logs/QuickTranslate-desktop.log` |

The same lines also go to stderr, so they show up in the `pnpm tauri dev` terminal.
