# Contributing

Thanks for taking a look. QuickTranslate is a small Swift Package Manager project with no Xcode
project file, so everything works from the command line.

## Development loop

```bash
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

## Layout

See "Project layout" in the README. Rough rules:

- AppKit for the window/menu/hotkey plumbing, SwiftUI for the views.
- No third-party dependencies. The app must keep working without network access to anything other
  than what the `claude` / `codex` CLIs do themselves.
- No API keys, tokens or telemetry in the app. Authentication is the CLI's job.

## Adding a UI language

1. Create `Resources/<code>.lproj/Localizable.strings` (copy `ko.lproj` as a starting point).
   Keys are the English strings; anything missing falls back to English.
2. Add the language to `AppSettings.uiLanguages` in `Sources/QuickTranslate/Settings.swift`.
3. Run `plutil -lint Resources/*/Localizable.strings`.

## Adding a translation language

Add the English name to `AppSettings.languages`, its localized names to each `.strings` file, the
`NLLanguage` mapping in `LanguageDetector.swift`, and the locale code mapping in
`AppSettings.systemDefaultLanguages()`.

## Pull requests

- Keep PRs focused; one change per PR.
- Describe how you tested it (which CLI, which macOS version) and paste relevant log lines.
- Run `swift build -c release` before pushing; there is no CI yet.

## Reporting bugs

Include your macOS version, which engine (Claude / Codex) and model, the output of
`claude --version` or `codex --version`, and the last ~20 lines of `~/Library/Logs/QuickTranslate.log`.
