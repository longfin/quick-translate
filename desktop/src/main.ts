// The translation panel. Its state (text, languages, pin) lives in Rust (`src-tauri/src/panel.rs`);
// this file renders it and streams the engine's `translate://*` events of the current id.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
import { t } from "./i18n";

interface View {
  id: number;
  text: string;
  source: string | null;
  detected: string | null;
  target: string;
  fallback: string;
  empty: boolean;
  engine: string;
}

interface Init {
  languages: string[];
  pinned: boolean;
  isMac: boolean;
  view: View | null;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const sourceSelect = $<HTMLSelectElement>("source");
const targetSelect = $<HTMLSelectElement>("target");
const swapButton = $<HTMLButtonElement>("swap");
const pinButton = $<HTMLButtonElement>("pin");
const sourceText = $("source-text");
const output = $("output");
const engineLabel = $("engine");
const status = $("status");
const cancelButton = $<HTMLButtonElement>("cancel");
const retranslateButton = $<HTMLButtonElement>("retranslate");
const copyButton = $<HTMLButtonElement>("copy");

const AUTO = "";
let view: View | null = null;
let translated = "";
let loading = false;
let error: string | null = null;
let cancelled = false;
let firstDelta = true;
let pinned = false;
let isMac = false;
let copiedTimer: number | undefined;

// ---- static strings -------------------------------------------------------------------------

for (const el of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
  el.textContent = t(el.dataset.i18n!);
}
for (const el of document.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
  el.title = t(el.dataset.i18nTitle!);
}

function fillLanguages(languages: string[]) {
  const option = (value: string, label: string) => {
    const o = document.createElement("option");
    o.value = value;
    o.textContent = label;
    return o;
  };
  sourceSelect.replaceChildren(option(AUTO, t("Auto-detect")));
  targetSelect.replaceChildren();
  for (const lang of languages) {
    sourceSelect.append(option(lang, t(lang)));
    targetSelect.append(option(lang, t(lang)));
  }
}

// ---- rendering ------------------------------------------------------------------------------

/** The source language the engine was given: picked, else detected, else unknown. */
const effectiveSource = () => view?.source ?? view?.detected ?? null;

function render() {
  const v = view;
  const src = effectiveSource();
  sourceSelect.value = v?.source ?? AUTO;
  sourceSelect.options[0].textContent =
    v?.detected && !v.source ? `${t("Auto-detect")} (${t(v.detected)})` : t("Auto-detect");
  if (v) targetSelect.value = v.target;
  for (const o of targetSelect.options) o.disabled = o.value === src;
  swapButton.disabled = !src || !v || v.empty;
  sourceSelect.disabled = targetSelect.disabled = !v || v.empty;

  sourceText.textContent = v?.text ?? "";
  output.classList.remove("error", "dim");
  if (v?.empty) {
    output.classList.add("dim");
    output.textContent = t(
      isMac
        ? "Nothing to translate. Select text and press ⌘C twice."
        : "Nothing to translate. Select text and press Ctrl+C twice.",
    );
  } else if (error) {
    output.classList.add("error");
    output.textContent = `⚠ ${error}`;
  } else if (!translated && loading) {
    output.classList.add("dim");
    output.textContent = t("Translating…");
  } else if (!translated && cancelled) {
    output.classList.add("dim");
    output.textContent = t("Cancelled");
  } else {
    output.textContent = translated;
  }
  engineLabel.textContent = v?.engine ?? "";
  cancelButton.hidden = !loading;
  retranslateButton.disabled = !v || v.empty;
  copyButton.disabled = !translated || !!error;
}

function setStatus(text: string) {
  status.textContent = text;
}

function start(v: View) {
  view = v;
  translated = "";
  error = null;
  cancelled = false;
  loading = !v.empty && v.id !== 0;
  firstDelta = true;
  setStatus("");
  render();
  output.scrollTop = 0;
  sourceText.scrollTop = 0;
}

function setPinned(value: boolean) {
  pinned = value;
  pinButton.setAttribute("aria-pressed", String(value));
  pinButton.title = value ? t("Unpin") : t("Pin window (stays open when clicking outside)");
}

// ---- events from Rust -----------------------------------------------------------------------

// Timing log lines (trigger → first delta). Not tied to requestAnimationFrame: WebKit pauses it
// for occluded windows, and the line should still show up.
const mark = (what: string) => void invoke("panel_mark", { what });

void listen<View>("panel://start", (e) => start(e.payload));

void listen<{ id: number; text: string }>("translate://delta", (e) => {
  if (!view || e.payload.id !== view.id) return;
  translated += e.payload.text;
  render();
  if (firstDelta) {
    firstDelta = false;
    mark("first delta received");
  }
});

void listen<{ id: number; text: string }>("translate://finished", (e) => {
  if (!view || e.payload.id !== view.id) return;
  if (e.payload.text) translated = e.payload.text;
  loading = false;
  render();
  mark(`finished, ${translated.length} chars`);
});

void listen<{ id: number; message: string }>("translate://failed", (e) => {
  if (!view || e.payload.id !== view.id) return;
  loading = false;
  error = e.payload.message;
  render();
  mark("error shown");
});

// ---- controls -------------------------------------------------------------------------------

function retranslate(source: string | null, target: string) {
  void invoke("panel_retranslate", { source, target });
}

sourceSelect.addEventListener("change", () => {
  if (!view) return;
  const source = sourceSelect.value || null;
  // Picking the target as source: swap instead of translating into the same language.
  const target = source === view.target ? (effectiveSource() ?? view.fallback) : view.target;
  retranslate(source, target);
});

targetSelect.addEventListener("change", () => {
  if (view) retranslate(view.source, targetSelect.value);
});

swapButton.addEventListener("click", () => {
  const src = effectiveSource();
  if (view && src) retranslate(view.target, src);
});

pinButton.addEventListener("click", () => {
  setPinned(!pinned);
  void invoke("panel_set_pinned", { pinned });
});

cancelButton.addEventListener("click", () => {
  void invoke("cancel");
  loading = false;
  cancelled = true;
  render();
  if (translated) setStatus(t("Cancelled"));
});

retranslateButton.addEventListener("click", () => {
  if (view) retranslate(view.source, view.target);
});

copyButton.addEventListener("click", async () => {
  if (!translated) return;
  try {
    await invoke("panel_copy", { text: translated });
    setStatus(t("Copied"));
    window.clearTimeout(copiedTimer);
    copiedTimer = window.setTimeout(() => setStatus(""), 1500);
  } catch (err) {
    setStatus(String(err));
  }
});

$("close").addEventListener("click", () => void invoke("panel_hide"));

window.addEventListener("keydown", (e) => {
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (e.key === "Escape" || (mod && e.key.toLowerCase() === "w")) {
    e.preventDefault();
    void invoke("panel_hide");
  } else if (mod && e.key === "Enter") {
    e.preventDefault();
    retranslateButton.click();
  }
});

// ---- theme ----------------------------------------------------------------------------------
// `prefers-color-scheme` covers most webviews; the window theme also lands in `data-theme` for
// those that don't pass the system setting through (some WebKitGTK setups).

const win = getCurrentWindow();
const applyTheme = (theme: Theme | null) => {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
};
void win.theme().then(applyTheme);
void win.onThemeChanged((e) => applyTheme(e.payload));

// ---- init -----------------------------------------------------------------------------------

void invoke<Init>("panel_init").then((init) => {
  isMac = init.isMac;
  fillLanguages(init.languages);
  setPinned(init.pinned);
  if (init.view && !view) start(init.view);
  else render();
});
