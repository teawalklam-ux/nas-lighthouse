import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Overlay } from "./Overlay";
import { OverlayPop } from "./OverlayPop";

function isOverlayWindow(): boolean {
  try {
    const params = new URLSearchParams(window.location.search);
    if (params.get("window") === "overlay") return true;
    return window.location.hash === "#overlay";
  } catch {
    return false;
  }
}

async function boot() {
  let label = "main";
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    label = getCurrentWindow().label;
  } catch {
    if (isOverlayWindow()) label = "overlay";
  }
  const overlay = label === "overlay";
  const pop = label === "pop";
  if (overlay) {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("overlay-root");
  }
  if (pop) {
    document.documentElement.classList.add("pop-root");
    document.body.classList.add("pop-root");
  }
  const root = document.getElementById("root") as HTMLElement;
  ReactDOM.createRoot(root).render(
    <React.StrictMode>{overlay ? <Overlay /> : pop ? <OverlayPop /> : <App />}</React.StrictMode>,
  );
}

void boot();
