// The Settings window. The settings live in Rust (`src-tauri/src/settings.rs`); every change is
// sent right away (no Save button, like the Swift app) and comes back as the new state, which
// `settings://changed` also brings when the file changed on disk or another window changed it.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
import { initI18n, t, type LocalePreference } from "./i18n";

type Backend = "claude" | "codex";

interface Settings {
  backend: Backend;
  claudeModel: string;
  codexModel: string;
  claudePath: string;
  codexPath: string;
  keepClaudeWarm: boolean;
  targetLanguage?: string;
  fallbackLanguage?: string;
  uiLanguage: LocalePreference;
  hotkeyDoublePress: boolean;
  hotkey: string;
  doublePressInterval: number;
  closeOnOutsideClick: boolean;
  launchAtLogin: boolean;
}

interface View {
  settings: Settings;
  target: string;
  fallback: string;
  languages: string[];
  clipboard: string;
  hotkeyError: string | null;
  path: string;
  platform: string;
}

interface CliInfo {
  name: string;
  path: string;
  version: string;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const backendSelect = $<HTMLSelectElement>("backend");
const modelLabel = $("model-label");
const modelInput = $<HTMLInputElement>("model");
const pathLabel = $("path-label");
const pathInput = $<HTMLInputElement>("cli-path");
const detected = $("detected");
const testButton = $<HTMLButtonElement>("test");
const testRow = $("test-row");
const testResult = $("test-result");
const warmRow = $("warm-row");
const keepWarm = $<HTMLInputElement>("keep-warm");
const targetSelect = $<HTMLSelectElement>("target");
const fallbackSelect = $<HTMLSelectElement>("fallback");
const uiLanguageSelect = $<HTMLSelectElement>("ui-language");
const triggerRadios = document.querySelectorAll<HTMLInputElement>('input[name="trigger"]');
const shortcutKbd = $("shortcut");
const recordButton = $<HTMLButtonElement>("record");
const intervalRow = $("interval-row");
const intervalInput = $<HTMLInputElement>("interval");
const intervalValue = $("interval-value");
const onceNote = $("once-note");
const hotkeyErrorRow = $("hotkey-error");
const clipboardNotice = $("clipboard-notice");
const closeOutside = $<HTMLInputElement>("close-outside");
const autostart = $<HTMLInputElement>("autostart");
const autostartErrorRow = $("autostart-error");
const settingsPath = $("settings-path");

let view: View | null = null;
let recording = false;
/** A recorder or registration problem the Rust side doesn't keep (e.g. a typing key). */
let recorderError: string | null = null;
let isMac = false;

// ---- strings --------------------------------------------------------------------------------

function applyStrings() {
  for (const el of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    el.textContent = t(el.dataset.i18n!);
  }
  document.title = t("QuickTranslate Settings");
}

function fillLanguages(select: HTMLSelectElement, languages: string[]) {
  if (select.options.length === languages.length) {
    for (const o of select.options) o.textContent = t(o.value);
    return;
  }
  select.replaceChildren(
    ...languages.map((lang) => {
      const o = document.createElement("option");
      o.value = lang;
      o.textContent = t(lang);
      return o;
    }),
  );
}

const formatInterval = (seconds: number) =>
  t("%.2fs").replace("%.2f", seconds.toFixed(2));

const doubleCopyLabel = () => (isMac ? "⌘C ⌘C" : "Ctrl+C Ctrl+C");

// ---- rendering ------------------------------------------------------------------------------

/** Text fields keep what the user is typing; everything else follows the state. */
function setText(input: HTMLInputElement, value: string) {
  if (document.activeElement !== input) input.value = value;
}

function render() {
  if (!view) return;
  const s = view.settings;
  const claude = s.backend === "claude";
  backendSelect.value = s.backend;
  modelLabel.textContent = t(claude ? "Model (haiku / sonnet / opus)" : "Model (blank = codex default)");
  pathLabel.textContent = t(claude ? "claude path (blank = auto-detect)" : "codex path (blank = auto-detect)");
  setText(modelInput, claude ? s.claudeModel : s.codexModel);
  setText(pathInput, claude ? s.claudePath : s.codexPath);
  warmRow.hidden = !claude;
  keepWarm.checked = s.keepClaudeWarm;

  fillLanguages(targetSelect, view.languages);
  fillLanguages(fallbackSelect, view.languages);
  targetSelect.value = view.target;
  fallbackSelect.value = view.fallback;
  uiLanguageSelect.value = s.uiLanguage;

  for (const r of triggerRadios) r.checked = (r.value === "double") === s.hotkeyDoublePress;
  if (recording) {
    shortcutKbd.textContent = t("Press the new shortcut…");
  } else {
    shortcutKbd.textContent = s.hotkeyDoublePress ? doubleCopyLabel() : s.hotkey;
  }
  shortcutKbd.classList.toggle("recording", recording);
  recordButton.textContent = recording ? t("Cancel") : t("Change…");
  recordButton.hidden = s.hotkeyDoublePress;
  intervalRow.hidden = !s.hotkeyDoublePress;
  intervalInput.value = String(s.doublePressInterval);
  intervalValue.textContent = formatInterval(s.doublePressInterval);
  onceNote.hidden = s.hotkeyDoublePress;

  const hotkeyError =
    recorderError ??
    (!s.hotkeyDoublePress && view.hotkeyError
      ? t("Couldn't register the shortcut: {error}", { error: view.hotkeyError })
      : null);
  showNote(hotkeyErrorRow, hotkeyError);
  clipboardNotice.hidden = !(s.hotkeyDoublePress && view.clipboard === "Unsupported");

  closeOutside.checked = s.closeOnOutsideClick;
  autostart.checked = s.launchAtLogin;
  settingsPath.textContent = view.path ? t("Settings file: {path}", { path: view.path }) : "";
}

function showNote(row: HTMLElement, text: string | null) {
  row.hidden = !text;
  row.querySelector(".note")!.textContent = text ?? "";
}

function setView(next: View) {
  const languageChanged = !view || view.settings.uiLanguage !== next.settings.uiLanguage;
  view = next;
  isMac = next.platform === "macos";
  if (languageChanged) {
    initI18n(next.settings.uiLanguage);
    applyStrings();
  }
  render();
}

// ---- changes --------------------------------------------------------------------------------

/** Sends `patch` (Swift key names); `errorRow` shows a failure next to the control. */
async function update(patch: Partial<Record<keyof Settings, unknown>>, errorRow?: HTMLElement) {
  try {
    setView(await invoke<View>("settings_update", { patch }));
    if (errorRow) showNote(errorRow, null);
  } catch (err) {
    console.error("settings_update", patch, err);
    if (errorRow) showNote(errorRow, String(err));
    setView(await invoke<View>("settings_get"));
  }
}

/** Debounced update for text fields, so typing doesn't save (and restart Claude) per key. */
function debounced(fn: () => void, ms: number): () => void {
  let timer: number | undefined;
  return () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(fn, ms);
  };
}

const currentBackend = (): Backend => (backendSelect.value === "codex" ? "codex" : "claude");

const saveModel = () =>
  update({ [currentBackend() === "claude" ? "claudeModel" : "codexModel"]: modelInput.value.trim() });
const savePath = () => {
  void update({ [currentBackend() === "claude" ? "claudePath" : "codexPath"]: pathInput.value.trim() });
  refreshDetected();
};
const saveModelSoon = debounced(() => void saveModel(), 600);
const savePathSoon = debounced(savePath, 600);

// The engine's CLI as it would be found now (Swift `refreshDetectedPath`).
const refreshDetected = debounced(async () => {
  const found = await invoke<string | null>("settings_detect_cli", {
    backend: currentBackend(),
    path: pathInput.value.trim(),
  });
  detected.classList.toggle("error", !found);
  detected.textContent = found ?? t("CLI not found");
}, 250);

backendSelect.addEventListener("change", async () => {
  testRow.hidden = true;
  await update({ backend: currentBackend() });
  refreshDetected();
});
modelInput.addEventListener("input", saveModelSoon);
modelInput.addEventListener("change", () => void saveModel());
pathInput.addEventListener("input", savePathSoon);
pathInput.addEventListener("change", savePath);
keepWarm.addEventListener("change", () => void update({ keepClaudeWarm: keepWarm.checked }));

testButton.addEventListener("click", async () => {
  testButton.disabled = true;
  testRow.hidden = false;
  testResult.className = "note mono";
  testResult.textContent = t("Testing…");
  try {
    const info = await invoke<CliInfo>("test_cli", {
      backend: currentBackend(),
      path: pathInput.value.trim(),
    });
    testResult.classList.add("ok");
    testResult.textContent = `✓ ${info.version}\n${info.path}`;
  } catch (err) {
    testResult.classList.add("error");
    testResult.textContent = `✗ ${String(err)}`;
  } finally {
    testButton.disabled = false;
  }
});

targetSelect.addEventListener("change", () => void update({ targetLanguage: targetSelect.value }));
fallbackSelect.addEventListener("change", () => void update({ fallbackLanguage: fallbackSelect.value }));
uiLanguageSelect.addEventListener("change", () => void update({ uiLanguage: uiLanguageSelect.value }));

for (const radio of triggerRadios) {
  radio.addEventListener("change", () => {
    if (!radio.checked) return;
    stopRecording();
    recorderError = null;
    void update({ hotkeyDoublePress: radio.value === "double" });
  });
}

intervalInput.addEventListener("input", () => {
  intervalValue.textContent = formatInterval(Number(intervalInput.value));
});
intervalInput.addEventListener("change", () => void update({ doublePressInterval: Number(intervalInput.value) }));

closeOutside.addEventListener("change", () => void update({ closeOnOutsideClick: closeOutside.checked }));

autostart.addEventListener("change", async () => {
  try {
    setView(await invoke<View>("settings_update", { patch: { launchAtLogin: autostart.checked } }));
    showNote(autostartErrorRow, null);
  } catch (err) {
    showNote(autostartErrorRow, t("Couldn't change the startup setting: {error}", { error: String(err) }));
    setView(await invoke<View>("settings_get"));
  }
});

// ---- hotkey recorder ------------------------------------------------------------------------

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS", "AltGraph", "Super", "Hyper"]);

/** `KeyboardEvent.code` → the key part of `Ctrl+Shift+T` (same rule as Rust's `key_label`). */
function keyLabel(code: string): string {
  const m = /^(?:Key|Digit)(\w)$/.exec(code);
  return m ? m[1] : code;
}

/** The chord of a keydown, e.g. `Ctrl+Shift+T`, or null for a lone modifier. */
function chord(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key) || !e.code) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push(isMac ? "Cmd" : "Super");
  parts.push(keyLabel(e.code));
  return parts.join("+");
}

/** Like the Swift `isUsable`: Ctrl, Alt or Super, unless it is a function key. */
const usable = (e: KeyboardEvent) => e.ctrlKey || e.altKey || e.metaKey || /^F\d{1,2}$/.test(e.code);

function startRecording() {
  recording = true;
  recorderError = null;
  render();
  shortcutKbd.focus();
}

function stopRecording() {
  if (!recording) return;
  recording = false;
  render();
}

recordButton.addEventListener("click", () => (recording ? stopRecording() : startRecording()));

async function record(e: KeyboardEvent) {
  if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
    stopRecording(); // Esc cancels, like the Swift recorder
    return;
  }
  const binding = chord(e);
  if (!binding) return; // still holding modifiers
  if (!usable(e)) {
    recorderError = t("Use Ctrl, Alt or Super with the key, or a function key.");
    render();
    return;
  }
  recording = false;
  try {
    recorderError = null;
    setView(await invoke<View>("settings_set_hotkey", { binding }));
  } catch (err) {
    recorderError = t("Couldn't register the shortcut: {error}", { error: String(err) });
    render();
  }
}

// ---- keys -----------------------------------------------------------------------------------

window.addEventListener(
  "keydown",
  (e) => {
    if (recording) {
      e.preventDefault();
      e.stopPropagation();
      void record(e);
      return;
    }
    const mod = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === "Escape" || (mod && e.key.toLowerCase() === "w")) {
      e.preventDefault();
      void invoke("settings_hide");
    }
  },
  true,
);

// Leaving the window while recording cancels it.
window.addEventListener("blur", stopRecording);

// ---- theme ----------------------------------------------------------------------------------

const win = getCurrentWindow();
const applyTheme = (theme: Theme | null) => {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
};
void win.theme().then(applyTheme);
void win.onThemeChanged((e) => applyTheme(e.payload));

// ---- init -----------------------------------------------------------------------------------

void listen<View>("settings://changed", (e) => {
  setView(e.payload);
  refreshDetected();
});

void invoke<View>("settings_get").then((v) => {
  setView(v);
  refreshDetected();
});
