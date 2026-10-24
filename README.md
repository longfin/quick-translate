<p align="center"><img src="docs/logo.png" width="128" alt="QuickTranslate icon"></p>

# QuickTranslate

English · [한국어](README.ko.md)

A tiny menu bar / tray app for **macOS, Windows and Linux**: select text, press **⌘C twice** (**Ctrl+C
twice** on Windows and Linux) and a translation pops up next to your cursor, like the DeepL desktop
client. No API keys. It uses the **Claude Code CLI** (your Claude subscription) or the **Codex CLI**
(your ChatGPT subscription) that you already have installed and logged in.

It is one [Tauri 2](https://v2.tauri.app) app in [`desktop/`](desktop) for all three platforms.
Downloads are on the [releases page](https://github.com/longfin/quick-translate/releases) (tags
`desktop-v*`).

> Until 0.1 the macOS app was a separate Swift app. If you used it, see
> [Coming from the Swift app](#coming-from-the-swift-app).

## Privacy

The app sends the text you translate to the CLI you chose, which sends it to Anthropic (Claude) or
OpenAI (Codex) under your own account. Nothing else leaves your machine: no analytics, no crash
reporting, no API keys. Usage counts against your subscription like any other CLI use.

## Requirements

- At least one of these installed and logged in:
  - `claude` — [Claude Code](https://docs.claude.com/en/docs/claude-code) (run `claude` once to log in)
  - `codex` — [Codex CLI](https://github.com/openai/codex) (`codex login`)

  Installed through npm they also need **Node.js** (18 or later), found next to the CLI or on the
  `PATH`.
- **macOS** 13 or later, Apple silicon or Intel (one universal app).
- **Windows** 10 (1803 or later) or 11, x64 or ARM64. The app draws its UI with the Microsoft
  Edge **WebView2** runtime: preinstalled on Windows 11 and on up-to-date Windows 10; the installer
  downloads it if it is missing (the portable zip doesn't, get it from
  [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/)).
- **Linux** x86_64 or aarch64 with glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and
  later), on X11 or Wayland. The tray icon needs `libayatana-appindicator3-1` (the `.deb` pulls it
  in). **GNOME** has no tray of its own: it needs the AppIndicator extension, which Ubuntu ships
  enabled; elsewhere install `gnome-shell-extension-appindicator` and turn it on in Extensions.
  KDE Plasma, Xfce, Cinnamon and MATE work as they are.

## Install

**macOS**

Unzip `QuickTranslate-<version>-macos-universal.app.zip` and move `QuickTranslate.app` to
`/Applications`. The build is ad-hoc signed, not notarized, so Gatekeeper refuses a downloaded copy
until you clear the quarantine flag once:

```bash
xattr -d com.apple.quarantine /Applications/QuickTranslate.app
```

It lives in the menu bar only (no Dock icon). No permissions are needed: ⌘C ⌘C is detected from
the clipboard and the hotkey uses the system hot key API, neither of which needs Accessibility.

**Windows**

- **Installer** (`QuickTranslate-<version>-windows-x64-setup.exe`, or `-arm64-` for ARM PCs):
  installs for the current user, no administrator rights, and adds a Start menu entry.
- **Portable** (`…-windows-x64-portable.zip` / `-arm64-`): unzip anywhere and run
  `quicktranslate.exe`. Settings and the log still go to your user profile.

The builds are not code-signed, so SmartScreen says "Windows protected your PC" the first time:
click **More info → Run anyway**.

**Linux**

- **AppImage** (`QuickTranslate-<version>-linux-x86_64.AppImage` or `-aarch64`), any distro:
  ```bash
  chmod +x QuickTranslate-*.AppImage
  ./QuickTranslate-*.AppImage
  ```
  AppImages need FUSE 2 (`sudo apt install libfuse2`, `libfuse2t64` on Ubuntu 24.04). Without it,
  run it with `--appimage-extract-and-run`.
- **Debian / Ubuntu** (`QuickTranslate_<version>_amd64.deb` or `_arm64.deb`):
  ```bash
  sudo apt install ./QuickTranslate_*_amd64.deb   # then start "QuickTranslate" from the app menu
  ```

`SHA256SUMS` next to the downloads lists the checksums. To start it with your session, tick
**Run at startup** in Settings.

To build it yourself, see [`desktop/README.md`](desktop/README.md).

## Usage

On Windows and Linux read Ctrl for ⌘.

| Action | How |
|---|---|
| Open the translation panel | Select text, press **⌘C ⌘C** (twice within 0.4 s). Or a hotkey, see Settings |
| Translate the clipboard | Menu bar / tray icon → **Translate clipboard** |
| Change target language | Picker at the top of the panel (retranslates immediately) |
| Edit the source and retranslate | Edit the text at the top, then **⌘⏎** |
| Copy the translation | The Copy button |
| Close | **Esc**, **⌘W**, or click outside (not while a translation is still running) |
| Keep it open | 📌 pin button (also keeps it on top) |

A trigger while the panel is visible replaces the text and restarts the translation in place.

By default it translates into your system language, and if the text is already in that language it
translates into your next preferred language (or English). Change this in Settings.

## Settings

Menu bar / tray icon → **Settings…**. Every change is saved and applied at once.

- **Engine**: Claude (Claude Code CLI) or ChatGPT (Codex CLI)
- **Model**: for Claude `haiku` (default, fastest) / `sonnet` / `opus`; for Codex leave blank for the default
- **CLI path**: set it if auto-detection fails (e.g. `~/.local/bin/claude`); **Test** shows what it finds
- **Languages**: default target language and the fallback used when the text is already in it
- **Interface language**: follow the system, or English / 한국어 / 日本語
- **Trigger**: **Press twice** (⌘C ⌘C, with its interval) or **Press once**, a global shortcut
  (`Ctrl+Shift+T` by default) recorded from the keyboard. The shortcut translates what is already
  on the clipboard, so copy first.
- **Close when clicking outside**
- **Keep Claude running in the background**: on by default. A resident `claude` process (~240 MB RSS,
  ~110 MB memory footprint) skips the CLI start-up (~0.5 s) on every translation. Turn it off to spawn a fresh
  process per translation.
- **Run at startup**: a LaunchAgent (macOS), the HKCU `Run` key (Windows), an XDG autostart entry (Linux)

Settings are stored as JSON in `~/Library/Application Support/dev.longfin.quicktranslate/settings.json`
(macOS), `%APPDATA%\dev.longfin.quicktranslate\settings.json` (Windows) or
`~/.config/dev.longfin.quicktranslate/settings.json` (Linux). The keys are listed in
[`desktop/README.md`](desktop/README.md#settings); the running app picks up hand edits within a second.

## How the trigger works

⌘C ⌘C / Ctrl+C Ctrl+C is detected from the clipboard, not the keyboard: two copies within the
interval mean "translate". No keyboard hook and no permissions, and it keeps working while a
terminal or password field has Secure Keyboard Entry on. The app learns about clipboard changes by
polling `NSPasteboard.changeCount` every 60 ms on macOS (the pasteboard has no change
notification), from Windows' clipboard listener, from XFixes on X11, and from the data-control
protocol on Wayland compositors that have it (KDE Plasma, Sway, Hyprland and other wlroots ones).

**Wayland on GNOME.** GNOME has neither data-control nor (for apps like this one) global shortcuts.
When XWayland is running, the app watches its clipboard instead, which follows copies from
Wayland apps too. Without XWayland, use the tray's **Translate clipboard**, or bind a GNOME
keyboard shortcut (Settings → Keyboard → Custom Shortcuts) to `quicktranslate --translate`. The
log says which watcher is in use (`trigger: clipboard watcher = …`).

## Triggering from other tools

With the app running, `quicktranslate --translate` opens the panel for the clipboard and
`quicktranslate --settings` opens Settings (the second launch passes the request on and exits).
Raycast, Hammerspoon, a GNOME shortcut or any script can call it. The command is:

- macOS: `/Applications/QuickTranslate.app/Contents/MacOS/quicktranslate`
- Windows: `quicktranslate.exe` (`%LOCALAPPDATA%\QuickTranslate\` for the installer)
- Linux: `quicktranslate` (`/usr/bin/quicktranslate` from the `.deb`), or the AppImage itself

## Troubleshooting

- **"`claude` CLI not found"**: the app looks on the `PATH` and in the usual install places (npm's
  global folder, Volta, fnm, nvm, bun, scoop, Homebrew, `~/.local/bin`, …), then asks a login shell
  (`where.exe` on Windows). If yours is elsewhere, set **CLI path** in Settings and press **Test**
  (`command -v claude` in a terminal, `where claude` on Windows, shows the path). On Windows npm
  installs a `claude.cmd` shim; point the setting at it or at `claude.exe`.
- **Works in a terminal, not from the app**: apps started from Finder, the Dock or a launcher don't
  read your shell's rc files, so a `PATH` set there is missing. The app adds the CLI's own folder
  to the `PATH` it passes on, so an npm shim finds the `node` next to it (nvm, fnm). If `node`
  lives somewhere else, set the CLI path to a wrapper script or install Node system-wide.
- **"QuickTranslate is damaged" / "can't be opened" on macOS**: the quarantine flag is still set,
  see [Install](#install).
- **No tray icon on GNOME**: enable the AppIndicator extension (see Requirements) and log out and
  in. The icon is a white glyph made for dark panels; on a light panel it is hard to see.
- **⌘C ⌘C / Ctrl+C Ctrl+C does nothing**: check the log for the watcher (see Wayland above), or
  switch to the hotkey.
- **Logs**: `~/Library/Logs/QuickTranslate.log` (macOS),
  `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log` (Windows),
  `~/.local/state/QuickTranslate/QuickTranslate.log` (Linux, `$XDG_STATE_HOME` if set).

## Coming from the Swift app

Versions before 0.1 shipped a native Swift app on macOS (bundle id `dev.swen.QuickTranslate`). The
Tauri app replaces it ([#20](https://github.com/longfin/quick-translate/issues/20)):

- Quit the old app and delete it from `/Applications`; both react to ⌘C ⌘C.
- Settings are not carried over yet: set the engine, model, languages and trigger again.
- The old app needed Accessibility for some shortcuts; this one doesn't. You can remove the old
  entry under System Settings → Privacy & Security → Accessibility, and its login item.
- The scripting trigger is `quicktranslate --translate` instead of the
  `dev.swen.QuickTranslate.translate` distributed notification.
- Press-once shortcuts translate what is already on the clipboard; the Swift app sent ⌘C first.

## Localization

The UI is English by default and switches to Korean or Japanese automatically based on the system
language. On first launch the default translation languages follow the system language too. The
strings are in [`desktop/src/locales/`](desktop/src/locales), keyed by the English text; see
[CONTRIBUTING](CONTRIBUTING.md#adding-a-ui-language) to add a language.

## How it works

- Claude: runs `claude -p --output-format stream-json --include-partial-messages --tools "" --strict-mcp-config --setting-sources ""`
  and streams tokens into the panel. The flags skip your user settings, hooks and MCP servers so it starts fast
  and sends only a few hundred input tokens. Extended thinking is disabled (`MAX_THINKING_TOKENS=0`).
- With "Keep Claude running" on, a single `claude -p --input-format stream-json` process stays alive. Each
  translation is one user message; after the result the app sends `/clear`, which the CLI handles locally, so
  every translation starts from a fresh context while the process, connection and prompt cache stay warm.
  Cancelling or changing the model restarts the process.
- Codex: runs `codex exec -s read-only -o <file>` and reads the final message (no streaming).
- The panel is a webview window shown near the cursor without taking focus from the app you are in
  (a non-activating `NSPanel` on macOS).

## Project layout

```
desktop/    the app (Tauri 2): src-tauri/ (Rust), src/ (web UI, vanilla TypeScript), locales
docs/       logo.png, icon-1024.png
.github/    CI and the release workflow
```

The app's own layout is in [`desktop/README.md`](desktop/README.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
