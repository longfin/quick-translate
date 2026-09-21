# QuickTranslate

English · [한국어](README.ko.md)

A tiny macOS menu bar app: press **⌘C twice** and a translation of the selected text pops up next to
your cursor, like the DeepL desktop client. No API keys. It uses the **Claude Code CLI** (your Claude
subscription) or the **Codex CLI** (your ChatGPT subscription) that you already have installed and logged in.

## Privacy

The app sends the text you translate to the CLI you chose, which sends it to Anthropic (Claude) or
OpenAI (Codex) under your own account. Nothing else leaves your machine: no analytics, no crash
reporting, no API keys. Usage counts against your subscription like any other CLI use.

## Requirements

- macOS 13+, Swift 5.9+ toolchain (Xcode Command Line Tools are enough)
- At least one of these installed and logged in:
  - `claude` — [Claude Code](https://docs.claude.com/en/docs/claude-code) (run `claude` once to log in)
  - `codex` — [Codex CLI](https://github.com/openai/codex) (`codex login`)

## Build & install

```bash
./make-cert.sh        # optional, once: self-signed identity so rebuilds keep the Accessibility permission
./build.sh            # builds build/QuickTranslate.app
./build.sh --install  # copies to /Applications and launches it
```

On first launch the app asks for the **Accessibility** permission (System Settings → Privacy & Security →
Accessibility). It is required to detect the global ⌘C ⌘C hotkey. If you grant it after the app has started,
the app relaunches itself once so macOS starts delivering key events.

> With plain ad-hoc signing every rebuild changes the signature and silently invalidates the permission
> (the switch looks on but nothing works). `./build.sh --install` clears the stale entry so you get asked
> again. Run `./make-cert.sh` once to create a local "QuickTranslate Dev" certificate; `build.sh` then signs
> with it automatically and the permission survives rebuilds. An Apple developer identity also works:
> `CODESIGN_IDENTITY="Apple Development: ..." ./build.sh`.

## Usage

| Action | How |
|---|---|
| Open the translation popup | Select text, press **⌘C ⌘C** (twice within 0.4s, or hold ⌘ and tap C twice) |
| Change target language | Picker at the top of the popup (retranslates immediately) |
| Edit the source and retranslate | Edit the text at the top, then **⌘⏎** |
| Copy the translation | **⇧⌘C** or the Copy button |
| Close | **Esc**, ⌘W, or click outside (not while a translation is still running) |
| Cancel a running translation | **⌘.** or the Cancel button (partial text stays) |
| Keep it open | 📌 pin button |

By default it translates into your system language, and if the text is already in that language it
translates into your next preferred language (or English). Change this in Settings.

## Settings

Menu bar icon → **Settings…**

- **Engine**: Claude (Claude Code CLI) or ChatGPT (Codex CLI)
- **Model**: for Claude `haiku` (default, fastest) / `sonnet` / `opus`; for Codex leave blank for the default
- **CLI path**: set manually if auto-detection fails (e.g. `~/.local/bin/claude`)
- **Languages**: default target language and the fallback used when the text is already in it
- **Interface language**: follow the system, or force English / 한국어 / 日本語 (the app restarts to apply)
- **Double ⌘C interval**, **Close when clicking outside**

## When the hotkey does not fire

- **Secure Keyboard Entry**: terminals (iTerm2, Ghostty, cmux, …) and password fields turn this on, and macOS
  then blocks global key monitoring for every app. The menu bar menu and the Settings window show which
  app has it enabled. Turn it off in that app or use the hotkey from another app. If an app quits without
  releasing it, the state can get stuck; locking and unlocking the screen resets it.
- **Permission granted after launch**: macOS decides at launch whether a process receives key events, so the
  app relaunches itself automatically once the permission appears.

## Triggering from other tools

Raycast, Hammerspoon or any script can open the popup for the current clipboard:

```bash
osascript -l JavaScript -e 'ObjC.import("Foundation"); $.NSDistributedNotificationCenter.defaultCenter.postNotificationNameObject("dev.swen.QuickTranslate.translate", $())'
```

Logs go to `~/Library/Logs/QuickTranslate.log`.

## Localization

The UI is English by default and switches to Korean or Japanese automatically based on the system
language. On first launch the default translation languages follow the system language too. To add a
language, create `Resources/<lang>.lproj/Localizable.strings` (keys are the English strings).

## How it works

- A CGEvent tap watches for two ⌘C key-downs in quick succession, reads the clipboard string and shows a
  non-activating floating `NSPanel` near the mouse. The panel never steals focus from the app you are in.
- Claude: runs `claude -p --output-format stream-json --include-partial-messages --tools "" --strict-mcp-config --setting-sources ""`
  and streams tokens into the panel. The flags skip your user settings, hooks and MCP servers so it starts fast
  and sends only a few hundred input tokens.
- Codex: runs `codex exec -s read-only -o <file>` and reads the final message (no streaming).

## Project layout

```
Sources/QuickTranslate/
  main.swift                 entry point (menu bar only, no Dock icon)
  AppDelegate.swift          status item menu, permission flow, clipboard trigger, auto-relaunch
  DoubleCopyMonitor.swift    ⌘C ⌘C detection (CGEvent tap)
  SecureInput.swift          Secure Keyboard Entry detection
  TranslationPanel.swift     floating panel (placement, outside-click close, Esc)
  TranslationView.swift      panel UI (SwiftUI)
  TranslationViewModel.swift translation state / streaming
  TranslationEngine.swift    claude / codex invocation and output parsing
  ProcessJob.swift           child process runner with line-streamed stdout
  CLILocator.swift           finds the CLI binaries (Finder-launched apps have a minimal PATH)
  Settings.swift             UserDefaults-backed settings
  SettingsView.swift         settings window
  L10n.swift                 localization helper
Resources/
  Info.plist, en/ko/ja.lproj/Localizable.strings
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)
