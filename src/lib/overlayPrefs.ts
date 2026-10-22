import type { AppConfig, OverlayDisplay } from "./types";

export const defaultOverlayDisplay: OverlayDisplay = {
  showLabels: true,
  precision: 1,
  showLink: true,
  showInternet: true,
  showCpu: true,
  showCpuTemp: true,
  showMem: true,
  showMemTemp: false,
  showNet: true,
  showAccess: true,
  showPorts: true,
  snapHide: true,
  opaque: false,
};

export function readOverlayDisplay(config?: AppConfig | null): OverlayDisplay {
  const raw = config?.ui.overlay?.display;
  return {
    ...defaultOverlayDisplay,
    ...raw,
    precision: Math.min(2, Math.max(0, raw?.precision ?? defaultOverlayDisplay.precision)),
  };
}

export function overlayDock(config?: AppConfig | null): "left" | "right" | null {
  const dock = config?.ui.overlay?.dock;
  return dock === "left" || dock === "right" ? dock : null;
}

/** 贴边且开启贴边收纳时，悬浮窗是 bot 小框；其余时候都是大框。 */
export function overlayIsMini(config?: AppConfig | null): boolean {
  return overlayDock(config) != null && readOverlayDisplay(config).snapHide;
}
