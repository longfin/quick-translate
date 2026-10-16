# QuickTranslate desktop (Tauri 2) — feasibility spike

Throwaway prototype answering "could Tauri replace the native Swift macOS app?" (issue #3).
Results: [`spike-notes/REPORT.md`](spike-notes/REPORT.md).

```bash
pnpm install
pnpm tauri dev      # or: pnpm tauri build  → src-tauri/target/release/bundle/macos/QuickTranslateSpike.app
```

Needs a logged-in `claude` CLI. Measurement switches (env vars, e.g. `open --env QT_TRIGGER=single …`):

| Var | Effect |
| --- | --- |
| `QT_PANEL=window` | keep a plain Tauri window instead of converting it to a non-activating `NSPanel` |
| `QT_TRIGGER=single` | fire on every pasteboard change (for scripted `pbcopy` runs) instead of ⌘C ⌘C |
| `QT_NO_PREWARM=1` | don't spawn the resident Claude worker at launch |
| `QT_LOG=path` | log file (default `~/Library/Logs/QuickTranslateSpike.log`) |
