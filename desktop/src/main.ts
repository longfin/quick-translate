import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const $ = (id: string) => document.getElementById(id)!;
const source = $("source");
const output = $("output");
const status = $("status");

let firstDelta = true;

const hide = () => invoke("hide_panel");

listen<string>("tr-start", (e) => {
  source.textContent = e.payload;
  output.textContent = "";
  output.classList.remove("error");
  status.textContent = "Translating…";
  firstDelta = true;
  requestAnimationFrame(() => invoke("mark", { what: "start painted" }));
});

listen<string>("tr-delta", (e) => {
  output.textContent += e.payload;
  if (firstDelta) {
    firstDelta = false;
    requestAnimationFrame(() => invoke("mark", { what: "first delta painted" }));
  }
});

listen<string>("tr-done", (e) => {
  output.textContent = e.payload;
  status.textContent = "";
  requestAnimationFrame(() => invoke("mark", { what: "done painted" }));
});

listen<string>("tr-error", (e) => {
  output.textContent = e.payload;
  output.classList.add("error");
  status.textContent = "";
});

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") hide();
});
$("close").addEventListener("click", hide);
