import type { ItemHealth, MetricsSnapshot } from "./types";
import type { AlertSummary } from "./alerts";
import type { StateId } from "../vendor/bloub/states";

export type BloubPin = "auto" | StateId;

export type BloubPlay = {
  hover?: boolean;
  opening?: boolean;
};

export type BloubMood = {
  state: StateId;
  reason: string;
  ink: string;
};

export const BLOUB_CHOICES: { id: BloubPin; label: string; hint: string }[] = [
  { id: "auto", label: "跟随状态", hint: "默认待机，悬停留意，打开主页环绕，异常时提示" },
  { id: "idle", label: "空闲", hint: "采集正常" },
  { id: "thinking", label: "思考", hint: "重连或正在采集" },
  { id: "wink", label: "探测", hint: "正在检查服务" },
  { id: "wide", label: "留意", hint: "鼠标停在动效上" },
  { id: "notify", label: "提醒", hint: "出站或个别服务异常" },
  { id: "alert", label: "断联", hint: "本机与 NAS 断开" },
  { id: "exclaim", label: "告警", hint: "阵列或关键异常" },
  { id: "sleep", label: "休眠", hint: "尚无数据" },
  { id: "orbit", label: "环绕", hint: "正在打开主页面" },
];

const PAGE_INK: Partial<Record<StateId, string>> = {
  idle: "#6d5dfc",
  thinking: "#7b6ef5",
  wink: "#6d5dfc",
  wide: "#2fbfa0",
  notify: "#c4a035",
  alert: "#e24b4b",
  exclaim: "#e24b4b",
  sleep: "#6b7280",
  orbit: "#6d5dfc",
};

const OVERLAY_INK: Partial<Record<StateId, string>> = {
  idle: "#dcff83",
  thinking: "#e7ffa3",
  wink: "#dcff83",
  wide: "#a0ffdd",
  notify: "#ffe49a",
  alert: "#ffb0a6",
  exclaim: "#ffb0a6",
  sleep: "#e8f0dc",
  orbit: "#dcff83",
};

export function parseBloubPin(raw?: string | null): BloubPin {
  if (!raw) return "auto";
  return BLOUB_CHOICES.some((item) => item.id === raw) ? (raw as BloubPin) : "auto";
}

export function bloubPaper(variant: "page" | "overlay"): string {
  return variant === "overlay" ? "#161814" : "#e0e5ec";
}

export function bloubInk(state: StateId, variant: "page" | "overlay"): string {
  const table = variant === "overlay" ? OVERLAY_INK : PAGE_INK;
  return table[state] ?? (variant === "overlay" ? OVERLAY_INK.idle! : PAGE_INK.idle!);
}

function moodOf(state: StateId, reason: string): BloubMood {
  return { state, reason, ink: PAGE_INK[state] ?? PAGE_INK.idle! };
}

export function moodFromStatus(
  snap: MetricsSnapshot | null,
  health: Record<string, ItemHealth>,
  alerts?: AlertSummary,
): BloubMood {
  if (!snap) {
    return moodOf("sleep", "尚未连上采集");
  }
  if (snap.link === "disconnected") {
    return moodOf("alert", "本机与 NAS 已断联");
  }
  if (snap.link === "reconnecting") {
    return moodOf("thinking", "正在重连 NAS 采集");
  }
  const raid = snap.raids.find((item) => !item.healthy);
  if (raid) {
    return moodOf("exclaim", raid.recovering ? `${raid.name} 正在重建` : `${raid.name} 阵列异常`);
  }
  if (alerts?.critical) {
    return moodOf("exclaim", `${alerts.critical} 项异常需要关注`);
  }
  if (alerts?.warning) {
    return moodOf("notify", `${alerts.warning} 项状态需要留意`);
  }
  if (snap.internet.state === "down") {
    return moodOf("notify", "NAS 无法出站访问互联网");
  }
  const down = Object.values(health).filter((item) => item.state === "down");
  if (down.length > 0) {
    return moodOf("notify", `${down.length} 个已收录服务探测失败`);
  }
  const degraded = Object.values(health).filter((item) => item.state === "degraded");
  if (degraded.length > 0) {
    return moodOf("notify", `${degraded.length} 个已收录服务需要留意`);
  }
  const unknown = Object.values(health).filter((item) => item.state === "unknown");
  if (unknown.length > 0 && Object.keys(health).length > 0) {
    return moodOf("wink", "正在探测已收录服务");
  }
  return moodOf("idle", "待机中");
}

export function resolveBloub(
  pin: BloubPin,
  snap: MetricsSnapshot | null,
  health: Record<string, ItemHealth>,
  variant: "page" | "overlay",
  play: BloubPlay = {},
  alerts?: AlertSummary,
): BloubMood {
  if (pin !== "auto") {
    const choice = BLOUB_CHOICES.find((item) => item.id === pin);
    return {
      state: pin,
      reason: choice?.hint ?? "",
      ink: bloubInk(pin, variant),
    };
  }
  const prompt = moodFromStatus(snap, health, alerts);
  if (prompt.state !== "idle") {
    return { ...prompt, ink: bloubInk(prompt.state, variant) };
  }
  if (play.opening) {
    return { state: "orbit", reason: "正在打开主页面", ink: bloubInk("orbit", variant) };
  }
  if (play.hover) {
    return { state: "wide", reason: "留意中", ink: bloubInk("wide", variant) };
  }
  return { ...prompt, ink: bloubInk("idle", variant) };
}
