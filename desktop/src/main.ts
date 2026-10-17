// Translation panel (Phase 3). For now the pre-created window is never shown;
// this file only proves the Vite + TypeScript + Tauri API wiring builds.
import { getCurrentWindow } from "@tauri-apps/api/window";

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    void getCurrentWindow().hide();
  }
});
