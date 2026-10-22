import { ago, dash } from "./format";
import type { MetricsSnapshot } from "./types";

export function linkTitle(snap: MetricsSnapshot): string {
  if (snap.link === "connected") return "采集流正常";
  return `上次成功 ${ago(snap.lastSuccessTs)}`;
}

export function internetTitle(snap: MetricsSnapshot): string {
  const latency = dash(snap.internet.latencyMs, (n) => `${n} ms`);
  const agent =
    snap.internet.linkAgentAlive === null
      ? "远程访问通道状态未知"
      : snap.internet.linkAgentAlive
        ? "Link agent 进程在运行（不能代表远程访问一定可用）"
        : "Link agent 进程未检测到（不能代表远程访问一定不可用）";
  if (snap.internet.state === "up") {
    return `NAS 已连接互联网，探测延迟 ${latency}。远程访问通道可用性无法确认。${agent}`;
  }
  if (snap.internet.state === "down") {
    return `NAS 无法出站访问互联网。远程访问通道可用性无法确认。${agent}`;
  }
  return `尚未完成出站探测。远程访问通道可用性无法确认。${agent}`;
}
