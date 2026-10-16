# Tauri on macOS — feasibility spike report

**Question (issue #3):** could a Tauri 2 app eventually replace the native Swift macOS app without a noticeable quality loss?

**Setup:** macOS 27.0 (Apple Silicon), Rust 1.98.0, Tauri 2.12.4 (wry 0.57, tao 0.37), tauri-nspanel 2.1.1 (git `v2.1`), objc2 0.6 / objc2-app-kit 0.3, vanilla TypeScript + Vite 8. `claude` CLI at `~/.local/bin/claude`, model `haiku`, same args, system prompt and stream-json parsing as `ClaudeWorker.swift`. All numbers come from the release `.app` launched with `open`.

## What was built (all of it works)

| Feature | Status | Notes |
| --- | --- | --- |
| Tray-only app | ✅ | Template tray icon; menu "Translate clipboard" / "Quit". `ActivationPolicy::Accessory` + `LSUIElement` gives `lsappinfo type="UIElement"`, so no Dock icon and no Dock flash. |
| ⌘C ⌘C trigger | ✅ | `NSPasteboard.changeCount` polled every 50 ms on a background thread (objc2-app-kit), fires on 2 changes ≤ 0.4 s. A real double write 162 ms apart fired correctly. |
| Pre-created hidden panel | ✅ | Undecorated, transparent, always-on-top, all Spaces, 480×340, positioned near the cursor and clamped to the work area. |
| Non-activating `NSPanel` | ✅ | `tauri-nspanel` built cleanly the first time (0 attempts wasted): `to_panel()` + `add_style_mask(nonactivating_panel)` + floating level + `canJoinAllSpaces \| fullScreenAuxiliary`. |
| Esc / outside click | ✅ (Esc not automated) | Esc → `keydown` in JS → `hide_panel` command. The panel becomes key, so keystrokes reach it. Outside click = `WindowEvent::Focused(false)` → hide, unless a translation is still streaming (matches Swift). Pin: not implemented. |
| Resident Claude worker | ✅ | tokio `Child` with piped stdin/stdout, one turn at a time, `/clear` after each turn, kill-and-respawn when a turn is busy, deltas sent with `app.emit("tr-delta")`. Pre-warmed at launch. |
| Vibrancy | ✅ | `WebviewWindowBuilder::effects(Popover, Active, radius 12)` installs `NSVisualEffectView` (window-vibrancy 0.8.1) behind a transparent `WKWebView`; rounded corners and window shadow work. |
| Settings / localization | — | Skipped as agreed. |

## Numbers

### Build and size (Tauri only)

| Metric | Tauri spike |
| --- | --- |
| `pnpm tauri build` | ✅ succeeds, no warnings. Clean release build ≈ 1 m 30 s; after a 1-line change ≈ 54 s (LTO + `codegen-units=1`) |
| Incremental `cargo build` (debug) after a 1-line change | 4–5 s |
| `.app` bundle size | **4.3 MB** (the binary is 4.4 MB stripped; frontend assets ≈ 4 KB) |
| Rust crates (macOS, `cargo tree -e normal`, unique) | 226 (`Cargo.lock` lists 418 because it covers Windows and Linux too). The Swift app has 0 third-party dependencies. |
| npm packages | 4 direct (`@tauri-apps/api`, `@tauri-apps/cli`, `vite`, `typescript`), 20 installed in total, all dev-time except `@tauri-apps/api` |
| Disk used while building | `src-tauri/target` 4.3 GB, `node_modules` 69 MB |

### Side by side: installed Swift app vs. Tauri spike

Both apps were measured in the same session, 2026-10-17 ~03:15, with [`scripts/compare.py`](scripts/compare.py). Only one app was running at a time. The 5 cold launches alternated Swift, Tauri, Swift, Tauri… Both use the same `claude` CLI, model `haiku` (the Swift app's `claudeModel`; `keepClaudeWarm` is at its default, on), and the same 5 texts as `scripts/baseline.py`. Values are **median (range)**.

| Metric | Swift (installed `/Applications/QuickTranslate.app`) | Tauri spike | n per app |
| --- | --- | --- | --- |
| Cold start: `open` → status item created and ⌘C ⌘C armed | **110 ms** (97–119) | **188 ms** (160–197) | 5 |
| Idle memory, app process (panel hidden, worker warm, measured 10 s after launch) | RSS **58 MB** (57.8–58.0), footprint **14 MB** | RSS **97 MB** (97.0–97.6), footprint **25 MB** (25–26) | 5 |
| Idle memory, WebKit helpers (WebContent + GPU + Networking) | none | RSS **79 MB**, footprint **36 MB** (3 processes; same every run) | 5 |
| **Idle memory, total** | **58 MB RSS / 14 MB footprint, 1 process** | **176 MB RSS / 61 MB footprint, 4 processes** | 5 |
| Warm `claude` worker (CLI's own cost) | RSS 238 MB (233–238), footprint 110 MB (108–111) | RSS 238 MB (237–240), footprint 111 MB (110–112) | 5 |
| ⌘C ⌘C → trigger detected in app (pasteboard polling) | 48 ms (7–57) | 28 ms (0–49) | 10 |
| ⌘C ⌘C → panel on screen, **first show after launch** | **210 ms** (196–215) | **54 ms** (25–96) | 5 |
| ⌘C ⌘C → panel on screen, later shows (panel had been hidden by an outside click) | 47 / 73 ms | 48 ms | 2 / 1 ⚠️ |
| ⌘C ⌘C → first streamed token | **not measurable**: the Swift log has no first-token marker | 1143 ms (798–1199) for a fresh worker; 1055 ms (762–1934) warm | 5 + 5 |
| ⌘C ⌘C → finished, fresh worker (first turn after launch) | 1814 ms (1400–1852) | 1581 ms (1221–2128) | 5 |
| ⌘C ⌘C → finished, warm worker (after one warm-up turn) | 1453 ms (1177–2230) | 1620 ms (1085–2378) | 5 |
| ⌘C ⌘C → finished, all runs | **1622 ms** (1177–2230) | **1600 ms** (1085–2378) | 10 |

The same CLI driven with no app at all (`scripts/baseline.py`, earlier the same night) gave first token 1.00 s and finished 1.64 s. Tauri's own overhead from a Rust event to the token painted in the webview is ≈ 1.3 ms (1.2–14 ms).

**How to read it:**
- **Translation latency is a tie.** The spread between runs (±0.5 s) comes from the API and is far larger than any app difference.
- **Tauri shows its panel faster on the first trigger after launch.** Its webview is created and loaded at startup. The Swift app takes ≈ 200 ms the first time, likely the SwiftUI hosting view's first layout and render; after that it shows in ≈ 50–70 ms.
- **The Swift app launches ≈ 80 ms faster**, because Tauri creates the hidden webview up front.
- **Memory is the real gap.** The Tauri spike uses ≈ 4.4× the footprint (61 vs 14 MB) and ≈ 3× the RSS (176 vs 58 MB), spread across 4 processes. The `claude` worker (≈ 110 MB footprint) costs the same in both and dominates either app.

**Methodology differences and caveats:**
- **Trigger.** Both apps are triggered by two real `pbcopy` writes 120 ms apart, which is the user-facing ⌘C ⌘C path in both. The clock starts (t0) when the second `pbcopy` returns. The Swift app polls the pasteboard every 60 ms (±20 ms leeway); the spike polls every 50 ms.
- **Panel on screen.** Measured outside both apps by `scripts/waitwin.swift`, which polls `CGWindowListCopyWindowInfo` every ~1 ms for a floating window of that app. It is identical for both and needs no permissions.
- **Not enough later-show samples (⚠️).** Neither app can be told to hide its panel without Accessibility permission (Esc or an outside click), and no Accessibility permission was added. So "later show" samples exist only when you happened to click elsewhere between runs, and that row has n = 2 / 1.
- **Cold start uses different markers.**
  - Swift: from `open` to the `pasteboard watcher started` log line. It is written right after `setupStatusItem()` in `applicationDidFinishLaunching`.
  - Tauri: from `open` to `TRAY_READY`; its pasteboard watcher starts < 1 ms later.
  - On macOS 27 the status item doesn't appear in `CGWindowList`, so the window server could not be used for this.
  - The Swift app was launched with `open <path>` and the spike with `open -n <path>`.
- **Finished.** Swift: its `translate finished` log line, written on the main queue when the result arrives. Tauri: `RESULT_DONE`, written in the worker task when the result line arrives (the webview paints ≈ 1–15 ms later).
- **First token.** The Swift app logs no first-token marker. I did not instrument the installed app or try to infer the first token from screenshots. The bare-CLI baseline and the identical CLI arguments are the best stand-in.
- **The prompts differ slightly.** The Swift app detects the source language and sends "Translate from English into Korean:". The spike always sends "Translate into Korean, or into English if the text is already mostly Korean:". The system prompt and CLI arguments are identical.
- **The panels differ.** The Swift panel is 772×450 (your saved frame) with native titlebar and toolbar. The spike's panel is 480×340 and undecorated.
- **Memory depends on uptime.** These are fresh-launch numbers. Your long-running Swift instance measured earlier showed a 42 MB footprint, so footprint grows with uptime. The spike was not run for hours, so its long-run growth is unknown.
- **Your Swift app was restarted.** The harness stopped it with SIGTERM; its worker exited on stdin EOF, as the spike's does. I relaunched it at the end (pid 13594, warm worker 13597).

## Focus behavior

| Mode | Frontmost app 300 ms after the panel appears |
| --- | --- |
| `tauri-nspanel`, non-activating | **unchanged** in every run (Adobe Lightroom, cmux, Ghostty, Threads, Claude stayed frontmost). The panel still becomes key, so it gets Esc and loses focus on outside clicks. |
| Plain Tauri window (`QT_PANEL=window`, `show()` + `set_focus()`) | **`QuickTranslateSpike` became frontmost, so it steals focus.** Without `set_focus()` the window would not get Esc. |

So **`tauri-nspanel` is required** to match the Swift UX, and it works.

## Visual quality

- Screenshot: [`panel-nspanel.png`](panel-nspanel.png), dark mode, after a real translation, captured with `screencapture -l`. A window-only capture doesn't include the blurred backdrop, so the material shows as flat grey there. The view tree logged at startup confirms `NSVisualEffectViewTagged` under a transparent `WryWebView`.
- Dark mode works through `prefers-color-scheme` in CSS. Light mode was not exercised because I did not change your system appearance.
- No white first-paint flash was observed. The webview is loaded while the window is still hidden, and both the page and the window are transparent. This was not verified frame by frame.
- One small rough edge found and fixed: the first focusable element (the ✕ button) showed a WebKit focus ring whenever the panel became key. Setting `tabindex="-1"` fixed it. Expect more of these "it's a web page" details: text selection, the context menu, ⌘A scope, scroll bounce, and font rendering that is close to native but not SwiftUI.
- Not reproduced: the native unified titlebar/toolbar, the traffic-light buttons and resize handles from PR #2's "native window chrome". The spike panel is undecorated with a hand-made HTML header. Getting native titlebar chrome *and* a non-activating panel needs more `NSPanel` style-mask work (`titled | fullSizeContentView`), which I did not attempt.

## macOS rough edges

- **No permission prompts.** Reading the pasteboard and showing the panel triggered no TCC prompt on macOS 27, and no Accessibility permission is needed.
- **Code signing / Gatekeeper:** without a signing identity, `tauri build` produces a binary that is only *linker* ad-hoc signed. `spctl` reports "code has no resources but signature indicates they must be present". The bundle runs locally because it has no quarantine flag, but if it were downloaded it would probably get the "damaged" dialog. Fix: set `bundle.macOS.signingIdentity` (`"-"` for a proper ad-hoc bundle signature, or a Developer ID plus notarization). Your Swift app is also rejected by `spctl` (self-signed "QuickTranslate Dev"), so distribution needs Developer ID + notarization either way. Tauri has built-in support for that (`APPLE_ID`/`APPLE_TEAM_ID` environment variables).
- **Pasteboard race (applies to both apps):** the writer's `clearContents` bumps `changeCount` *before* the string is written. One poll landed in that gap and read an empty clipboard. Fixed in the spike with a short retry (10 × 20 ms). The Swift `PasteboardWatcher` has the same theoretical race.
- `Effect`/vibrancy and transparent windows need `macOSPrivateApi: true`. Tauri treats this as a private API and it makes the app ineligible for the Mac App Store. That is not relevant today.
- The `cargo` `target/` directory reaches 4.3 GB for one app.
- Killing the app with SIGTERM left no orphaned `claude` process. The CLI exits on stdin EOF, and the child also has `kill_on_drop` set.

## Developer experience

- Total spike code: Rust 640 lines (`lib.rs` 224, `worker.rs` 250, `macos.rs` 119, `log.rs` 41), TS 45, CSS 91, HTML 20.
- **macOS-specific Rust: `macos.rs` ≈ 120 lines with 2 `unsafe` blocks** (`stringForType`, and a cast of `ns_window()` used only for the debug view-tree dump). There are no hand-written `msg_send!` calls; `tauri-nspanel` and objc2-app-kit's typed bindings cover everything. The rest (tray, window, IPC, process worker) is cross-platform Tauri/tokio code that would carry straight over to Windows and Linux. Only the pasteboard poll and the `NSPanel` conversion are macOS-only.
- Iteration speed: frontend changes reload instantly with `tauri dev`. A Rust change takes ≈ 5 s in debug and ≈ 55 s for a release bundle. That is slower than `swift build` for a project this size, but fine.
- `tauri-nspanel` is a single-maintainer git dependency (not on crates.io for 2.1). It is a real but contained supply-chain/maintenance risk: ≈ 1.5 k lines of macros over objc2.

## Not done / not measured

- Pin button, settings, language detection, codex backend, Carbon single-press hotkey, launch at login (`tauri-plugin-autostart` exists), localization.
- Esc was not driven by automation, because that needs Accessibility permission for the terminal. It is wired the standard way (JS `keydown`, panel is key) (evidence that it is key: it receives `Focused(false)` when you click elsewhere). The close button was not clicked in testing either.
- Light mode, multi-monitor positioning on a non-primary display, and full-screen Spaces were not tested explicitly.

## Reproduce

`pnpm tauri build`, then use the scripts in `spike-notes/scripts/`: `compare.py <waitwin-binary>` (side by side with the installed Swift app; it restarts that app), `coldstart.sh`, `trigger.sh N` (launch the app with `open --env QT_TRIGGER=single …` first), `baseline.py` (bare CLI), and `winid.swift` (window id for `screencapture -l`). The log is written to `~/Library/Logs/QuickTranslateSpike.log`, with trigger-relative timestamps on every line.
