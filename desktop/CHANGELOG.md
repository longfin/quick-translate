# Changelog: QuickTranslate for Windows and Linux

The `desktop/` app only; the macOS app is versioned separately. Releases are tagged
`desktop-v<version>` ([`release.yml`](../.github/workflows/release.yml)).

## Unreleased (0.1.0)

First release, planned in [#3](https://github.com/longfin/quick-translate/issues/3).

- **Phase 0, scaffold**: Tauri 2 tray app with no window at startup, single instance, log file,
  CI for Windows and Linux.
- **Phase 1, engine**: the `claude` / `codex` CLI engine ported from the macOS app: streaming,
  the resident `claude` worker, cancellation of the whole process tree, CLI discovery including
  npm / volta / bun shims on Windows.
- **Phase 2, triggers**: Ctrl+C Ctrl+C detected from clipboard changes (Windows clipboard
  listener, X11 XFixes, Wayland data-control, XWayland), an optional single-press hotkey, the tray
  item and `quicktranslate --translate`.
- **Phase 3, translation panel**: undecorated panel near the cursor, streaming text, copy, pin,
  target language picker, close on Esc / Ctrl+W / clicking outside.
- **Phases 5 and 6, languages and localization**: source language detection, target / fallback
  language from the system languages, UI strings shared with the macOS app (English, Korean,
  Japanese).
- **Phase 4, Settings window**: engine, model, CLI path with a test button, languages, trigger
  and hotkey recorder, close on outside click, keep Claude warm, run at startup; applied live and
  picked up when `settings.json` is edited by hand.
- **Phase 7, packaging**: NSIS installer and portable zip (Windows x64 and ARM64), AppImage and
  `.deb` (Linux x86_64 and aarch64), a release workflow with SHA256SUMS, a monochrome tray icon,
  a headless Linux smoke test in CI, and docs.
