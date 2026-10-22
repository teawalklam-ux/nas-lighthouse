export function dash(
  value: number | null | undefined,
  format?: (n: number) => string,
): string {
  if (value === null || value === undefined || Number.isNaN(value)) {
    return "—";
  }
  return format ? format(value) : String(value);
}

export function pct(value: number, digits = 1): string {
  return `${value.toFixed(digits)}%`;
}

export function celsius(value: number, digits = 1): string {
  return `${value.toFixed(digits)}°C`;
}

export function bytesPerSec(value: number): string {
  const abs = Math.abs(value);
  if (abs < 1024) return `${value.toFixed(0)} B/s`;
  if (abs < 1024 ** 2) return `${(value / 1024).toFixed(1)} KB/s`;
  if (abs < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MB/s`;
  return `${(value / 1024 ** 3).toFixed(2)} GB/s`;
}

export function bytes(value: number): string {
  const abs = Math.abs(value);
  if (abs < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MB`;
  if (abs < 1024 ** 4) return `${(value / 1024 ** 3).toFixed(1)} GB`;
  return `${(value / 1024 ** 4).toFixed(1)} TB`;
}

export function kib(value: number): string {
  return bytes(value * 1024);
}

export function uptime(sec: number): string {
  const days = Math.floor(sec / 86400);
  const hours = Math.floor((sec % 86400) / 3600);
  const minutes = Math.floor((sec % 3600) / 60);
  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分`;
  return `${minutes} 分`;
}

export function ago(ts: number | null, now = Date.now()): string {
  if (!ts) return "尚无";
  const sec = Math.max(0, Math.round((now - ts) / 1000));
  if (sec < 5) return "刚刚";
  if (sec < 60) return `${sec} 秒前`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  return `${Math.floor(min / 60)} 小时前`;
}

export function durationBetween(start: number, end = Date.now()): string {
  const sec = Math.max(0, Math.round((end - start) / 1000));
  if (sec < 60) return `${sec} 秒`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟`;
  const hours = Math.floor(min / 60);
  if (hours < 24) return `${hours} 小时 ${min % 60} 分`;
  const days = Math.floor(hours / 24);
  return `${days} 天 ${hours % 24} 小时`;
}

export function usedPct(used: number, total: number): number | null {
  if (!total) return null;
  return (used / total) * 100;
}

/** media-jellyfin-media-jellyfin-1 → media-jellyfin-1 */
export function compactContainerName(name: string): string {
  const split = name.lastIndexOf("-");
  if (split <= 0) return name;
  const body = name.slice(0, split);
  const replica = name.slice(split + 1);
  if (!/^\d+$/.test(replica)) return name;
  if (body.length % 2 === 0) return name;
  const prefixLen = (body.length - 1) / 2;
  if (body[prefixLen] !== "-") return name;
  const left = body.slice(0, prefixLen);
  const right = body.slice(prefixLen + 1);
  if (left && left === right) return `${left}-${replica}`;
  return name;
}

export function accessLabel(
  item: {
    title: string | null;
    port: number;
    containerName: string | null;
  },
  showPorts = true,
): string {
  const name =
    item.title && item.title !== `端口 ${item.port}`
      ? item.title
      : item.containerName
        ? compactContainerName(item.containerName)
        : null;
  if (!showPorts) return name ?? `端口 ${item.port}`;
  if (name) {
    if (name.endsWith(` · ${item.port}`) || name === `端口 ${item.port}`) return name;
    return `${name} · ${item.port}`;
  }
  return `端口 ${item.port}`;
}
