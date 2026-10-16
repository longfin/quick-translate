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

| Metric | Tauri spike | Swift app (reference) |
| --- | --- | --- |
| `pnpm tauri build` | ✅ succeeds, no warnings. Clean release build ≈ 1 m 30 s; after a 1-line change ≈ 54 s (LTO + `codegen-units=1`) | — |
| Incremental `cargo build` (debug) after a 1-line change | 4–5 s | — |
| `.app` bundle size | **4.3 MB** (the binary is 4.4 MB stripped; frontend assets ≈ 4 KB) | — |
| Rust crates (macOS, `cargo tree -e normal`, unique) | 226 (`Cargo.lock` lists 418 because it covers Windows and Linux too); `cargo tree` prints 678 lines | 0 third-party |
| npm packages | 4 direct (`@tauri-apps/api`, `@tauri-apps/cli`, `vite`, `typescript`), 20 installed in total, all dev-time except `@tauri-apps/api` | — |
| Disk used while building | `src-tauri/target` 4.3 GB, `node_modules` 69 MB | — |
| Cold start: `open` → tray icon ready | 174 / 222 / 199 ms when warm (first launch after a build: 753 ms, probably the first code-signature and LaunchServices registration). In-process: `main()` → `TRAY_READY` ≈ 130–170 ms, mostly creating the hidden webview (≈ 150 ms). | not measured (I did not relaunch your installed app) |
| Idle memory, app only (panel created and hidden) | RSS 96–100 MB; **phys_footprint 28 MB** | RSS 65 MB; **phys_footprint 42 MB** |
| Idle memory, WebKit helper processes (only the webview path pays this) | WebContent 35–39 MB RSS / 19 MB footprint, GPU 25–27 MB / 13 MB, Networking 10–15 MB / 7 MB | none (SwiftUI runs in process) |
| **Idle memory, total** | **≈ 165 MB RSS / ≈ 67 MB footprint across 4 processes** | **≈ 65 MB RSS / 42 MB footprint, 1 process** |
| Warm Claude worker (the CLI's own cost, the same for both apps) | 238–258 MB RSS / 133 MB footprint just after spawning | 174 MB RSS / 121 MB footprint after hours of uptime |
| Trigger → `orderFront` returned | 1–11 ms (median ≈ 3 ms) | — |
| Trigger → panel painted (JS `requestAnimationFrame` after the `tr-start` event) | 2–65 ms, **median ≈ 21 ms** (n = 10) | — |
| Trigger → first streamed token | **median ≈ 1.00 s** (0.83–1.16 s, n = 8) | — |
| Trigger → finished (2-sentence paragraph, ≈ 150 chars) | **median ≈ 1.63 s** (1.31–1.74 s, n = 9) | the claimed ~0.7 s was not reproducible today; see below |
| Same CLI with no Tauri involved (`scripts/baseline.py`, same 5 texts, same args) | first token median 1.00 s, finished median 1.64 s | — |
| Rust event → token painted in the webview (IPC + DOM + next frame) | **1.2–14 ms, typically ≈ 1.3 ms** | — |

**Reading the latency rows:** in this setup Tauri adds essentially nothing. The time is all CLI and API: the bare CLI harness gives the same 1.0 s / 1.64 s medians as the Tauri app. The Swift app's ~0.7 s figure was not reproducible today even with the bare CLI. Your Swift app's own log from earlier today shows 2.0–3.8 s for 325–1184-character inputs, which is consistent with what I saw. `/clear` returns its own `result` line ≈ 9 ms after it is sent. Both workers ignore that line, because they only accept a `result` while a turn is busy.

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

- Swift app cold start (I did not want to relaunch your installed app). Pin button, settings, language detection, codex backend, Carbon single-press hotkey, launch at login (`tauri-plugin-autostart` exists), localization.
- Esc was not driven by automation, because that needs Accessibility permission for the terminal. It is wired the standard way (JS `keydown`, panel is key) (evidence that it is key: it receives `Focused(false)` when you click elsewhere). The close button was not clicked in testing either.
- Light mode, multi-monitor positioning on a non-primary display, and full-screen Spaces were not tested explicitly.

## Reproduce

`pnpm tauri build`, then use the scripts in `spike-notes/scripts/`: `coldstart.sh`, `trigger.sh N` (launch the app with `open --env QT_TRIGGER=single …` first), `baseline.py` (bare CLI), and `winid.swift` (window id for `screencapture -l`). The log is written to `~/Library/Logs/QuickTranslateSpike.log`, with trigger-relative timestamps on every line.
