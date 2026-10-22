import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GlideSelect } from "../components/GlideSelect";
import { ToggleRow } from "../components/Toggle";
import { defaultOverlayDisplay, readOverlayDisplay } from "../lib/overlayPrefs";
import type { AppConfig, OverlayDisplay } from "../lib/types";

type OverlayFlag = Exclude<keyof OverlayDisplay, "precision">;

function titlesToText(rules: Record<string, string> | undefined): string {
  return Object.entries(rules ?? {})
    .map(([key, title]) => `${key}=${title}`)
    .join("\n");
}

function textToTitles(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const split = line.indexOf("=");
    if (split <= 0) continue;
    const key = line.slice(0, split).trim();
    const title = line.slice(split + 1).trim();
    if (key && title) out[key] = title;
  }
  return out;
}

function splitList(text: string): string[] {
  return text
    .split(/[\s,]+/)
    .map((part) => part.trim())
    .filter(Boolean);
}

function splitPorts(text: string): number[] {
  return splitList(text)
    .map((part) => Number(part))
    .filter((n) => Number.isInteger(n) && n > 0 && n < 65536);
}

const FIELDS: { key: OverlayFlag; label: string; hint: string }[] = [
  { key: "showLabels", label: "解释文字", hint: "如 CPU 1.3%，而不是只显示 1.3%" },
  { key: "showLink", label: "本机与 NAS 连接点", hint: "SSH 采集流状态" },
  { key: "showInternet", label: "NAS 出站点", hint: "出站探测，不是厂商远程访问通道" },
  { key: "showCpu", label: "CPU 占用", hint: "" },
  { key: "showCpuTemp", label: "CPU 温度", hint: "" },
  { key: "showMem", label: "内存占用", hint: "" },
  { key: "showMemTemp", label: "内存温度", hint: "" },
  { key: "showNet", label: "上下行网速", hint: "" },
  { key: "showAccess", label: "导航快捷入口", hint: "已收录的导航项，点击用浏览器打开" },
  { key: "showPorts", label: "显示端口号", hint: "有空间时显示「Jellyfin · 8096」；一行放不下会先收起端口" },
  { key: "snapHide", label: "贴边收成小窗", hint: "拖到左右边缘变成 bot 小框。关掉后立刻恢复大框" },
];

export function SettingsPage({
  config,
  onSave,
}: {
  config: AppConfig;
  onSave: (config: AppConfig) => Promise<void>;
}) {
  const [sshAlias, setSshAlias] = useState(config.sshAlias);
  const [nasHost, setNasHost] = useState(config.nasHost);
  const [everythingHttp, setEverythingHttp] = useState(config.search?.everythingHttp ?? "http://127.0.0.1:80");
  const [alistUrl, setAlistUrl] = useState(config.search?.alistUrl ?? "");
  const [alistContainer, setAlistContainer] = useState(config.search?.alistContainer ?? "alist");
  const [alistBackend, setAlistBackend] = useState<"alist" | "openlist">(config.search?.alistBackend ?? "alist");
  const [quarkMount, setQuarkMount] = useState(config.search?.quarkMount ?? "/夸克");
  const [baiduMount, setBaiduMount] = useState(config.search?.baiduMount ?? "/百度");
  const [everythingExecutable, setEverythingExecutable] = useState(config.search?.everythingExecutable ?? "");
  const [intervalMs, setIntervalMs] = useState(String(config.ui.intervalMs));
  const [netIface, setNetIface] = useState(config.collector?.netIface ?? "");
  const [diskMounts, setDiskMounts] = useState((config.collector?.diskMounts ?? []).join(", "));
  const [probeHosts, setProbeHosts] = useState((config.collector?.probeHosts ?? ["1.1.1.1", "8.8.8.8"]).join(" "));
  const [linkAgent, setLinkAgent] = useState(config.collector?.linkAgentProcess ?? "");
  const [watchPorts, setWatchPorts] = useState((config.watchPorts ?? []).join(", "));
  const [titleRules, setTitleRules] = useState(titlesToText(config.containerTitles));
  const [probe, setProbe] = useState(config.ui.internetProbeEnabled);
  const [autostart, setAutostart] = useState(Boolean(config.ui.autostart));
  const [autostartOs, setAutostartOs] = useState<boolean | null>(null);
  const [overlayOn, setOverlayOn] = useState(config.ui.overlay?.visible !== false);
  const [display, setDisplay] = useState<OverlayDisplay>(readOverlayDisplay(config));
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setProbe(config.ui.internetProbeEnabled);
    setAutostart(Boolean(config.ui.autostart));
    setOverlayOn(config.ui.overlay?.visible !== false);
    setDisplay(readOverlayDisplay(config));
    setEverythingHttp(config.search?.everythingHttp ?? "http://127.0.0.1:80");
    setAlistUrl(config.search?.alistUrl ?? "");
    setAlistContainer(config.search?.alistContainer ?? "alist");
    setAlistBackend(config.search?.alistBackend ?? "alist");
    setQuarkMount(config.search?.quarkMount ?? "/夸克");
    setBaiduMount(config.search?.baiduMount ?? "/百度");
    setEverythingExecutable(config.search?.everythingExecutable ?? "");
    setNetIface(config.collector?.netIface ?? "");
    setDiskMounts((config.collector?.diskMounts ?? []).join(", "));
    setProbeHosts((config.collector?.probeHosts ?? ["1.1.1.1", "8.8.8.8"]).join(" "));
    setLinkAgent(config.collector?.linkAgentProcess ?? "");
    setWatchPorts((config.watchPorts ?? []).join(", "));
    setTitleRules(titlesToText(config.containerTitles));
  }, [config]);

  useEffect(() => {
    void refreshAutostartOs();
  }, []);

  async function refreshAutostartOs() {
    try {
      setAutostartOs(await invoke<boolean>("autostart_status"));
    } catch {
      setAutostartOs(null);
    }
  }

  async function save() {
    setBusy(true);
    setMessage(null);
    try {
      const next = await invoke<AppConfig>("save_settings", {
        patch: {
          sshAlias,
          nasHost,
          intervalMs: Number(intervalMs),
          everythingHttp,
          alistUrl,
          alistContainer,
          alistBackend,
          quarkMount,
          baiduMount,
          everythingExecutable,
          netIface,
          diskMounts: diskMounts.split(/[,\n]+/).map((path) => path.trim()).filter(Boolean),
          probeHosts: splitList(probeHosts),
          linkAgentProcess: linkAgent,
          watchPorts: splitPorts(watchPorts),
          containerTitles: textToTitles(titleRules),
        },
      });
      await onSave(next);
      setMessage("已保存。采集间隔会立即重连 SSH 后生效。");
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function patchFlag(patch: Record<string, unknown>, ok: string) {
    setBusy(true);
    setMessage(null);
    try {
      const next = await invoke<AppConfig>("save_settings", { patch });
      setProbe(next.ui.internetProbeEnabled);
      setAutostart(Boolean(next.ui.autostart));
      await onSave(next);
      if ("autostart" in patch) {
        await refreshAutostartOs();
        const os = await invoke<boolean>("autostart_status").catch(() => null);
        if (next.ui.autostart && os === false) {
          setMessage(
            "配置已打开，但没有有效的开机启动项。开发调试版不能用来开机自启，请先打包安装后再打开此开关。",
          );
          return;
        }
        if (!next.ui.autostart && os === true) {
          setMessage("配置已关闭，但系统启动项仍在。");
          return;
        }
      }
      setMessage(ok);
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  async function patchOverlay(patch: Record<string, unknown>) {
    const next = await invoke<AppConfig>("set_overlay_state", { patch });
    setOverlayOn(next.ui.overlay.visible);
    setDisplay(readOverlayDisplay(next));
    await onSave(next);
  }

  async function exportConfig() {
    const json = await invoke<string>("export_config");
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "nas-lighthouse-config.json";
    link.click();
    URL.revokeObjectURL(url);
  }

  async function importFile(file: File) {
    setBusy(true);
    setMessage(null);
    try {
      const json = await file.text();
      const next = await invoke<AppConfig>("import_config", { json });
      setSshAlias(next.sshAlias);
      setNasHost(next.nasHost);
      setEverythingHttp(next.search?.everythingHttp ?? "http://127.0.0.1:80");
      setAlistUrl(next.search?.alistUrl ?? "");
      setAlistContainer(next.search?.alistContainer ?? "alist");
      setIntervalMs(String(next.ui.intervalMs));
      setNetIface(next.collector?.netIface ?? "");
      setDiskMounts((next.collector?.diskMounts ?? []).join(", "));
      setProbeHosts((next.collector?.probeHosts ?? []).join(" "));
      setLinkAgent(next.collector?.linkAgentProcess ?? "");
      setWatchPorts((next.watchPorts ?? []).join(", "));
      setTitleRules(titlesToText(next.containerTitles));
      setProbe(next.ui.internetProbeEnabled);
      setAutostart(Boolean(next.ui.autostart));
      setOverlayOn(next.ui.overlay.visible);
      setDisplay(readOverlayDisplay(next));
      await onSave(next);
      await refreshAutostartOs();
      setMessage(`已导入 ${next.navItems.length} 个导航项。`);
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>设置</h1>
        </div>
      </header>

      <p className="muted">配置备份包含主机地址、服务名称和导航链接，请勿直接附在公开 Issue 中。旧版配置需手动导入，开机自启单独设置。</p>
      <article className="card form-card">
        <h2>连接与采集</h2>
        <label>
          SSH 别名
          <input
            value={sshAlias}
            onChange={(e) => setSshAlias(e.target.value)}
            placeholder="SSH config 的 Host，例如 nas"
          />
        </label>
        <label>
          NAS 地址
          <input
            value={nasHost}
            onChange={(e) => setNasHost(e.target.value)}
            placeholder="用来拼服务 URL，例如 nas.local 或 192.0.2.10"
          />
        </label>
        <label>
          Everything HTTP（可选）
          <input
            value={everythingHttp}
            onChange={(e) => setEverythingHttp(e.target.value)}
            placeholder="http://127.0.0.1:80"
          />
        </label>
        <p className="muted">本机搜索优先用 es.exe。便携版 Everything 默认不开 HTTP，不必填 80 端口。</p>
        <label>
          AList 地址
          <input
            value={alistUrl}
            onChange={(e) => setAlistUrl(e.target.value)}
            placeholder="空则跳过 NAS 搜索与网盘挂载，例如 http://nas.local:5244"
          />
        </label>
        <label>
          AList 容器名
          <input
            value={alistContainer}
            onChange={(e) => setAlistContainer(e.target.value)}
            placeholder="docker exec 读取令牌用的容器名，例如 alist"
          />
        </label>
        <label>
          搜索后端类型
          <GlideSelect value={alistBackend} onChange={(value) => setAlistBackend(value as "alist" | "openlist")}
            options={[{ value: "alist", label: "AList" }, { value: "openlist", label: "OpenList（兼容模式）" }]} />
        </label>
        <label>夸克存储挂载路径<input value={quarkMount} onChange={(e) => setQuarkMount(e.target.value)} /></label>
        <label>百度存储挂载路径<input value={baiduMount} onChange={(e) => setBaiduMount(e.target.value)} /></label>
        <p className="muted">请先在 AList / OpenList 后台创建存储。此处只更新已有存储的凭据，其他设置保持原样。</p>
        <label>Everything 的 es.exe 路径（可空）<input value={everythingExecutable} onChange={(e) => setEverythingExecutable(e.target.value)} placeholder="空则在安装目录和 PATH 中查找" /></label>
        <label>
          网卡名（可空）
          <input
            value={netIface}
            onChange={(e) => setNetIface(e.target.value)}
            placeholder="空则跟随默认路由，例如 eth0 / bond0 / enp2s0"
          />
        </label>
        <label>
          磁盘挂载点（可空）
          <input
            value={diskMounts}
            onChange={(e) => setDiskMounts(e.target.value)}
            placeholder="空则自动列出；多个路径用逗号分隔，例如 /mnt/data, /mnt/media"
          />
        </label>
        <label>
          出站探测主机
          <input value={probeHosts} onChange={(e) => setProbeHosts(e.target.value)} />
        </label>
        <label>
          远程访问 agent 进程名（可空）
          <input
            value={linkAgent}
            onChange={(e) => setLinkAgent(e.target.value)}
            placeholder="例如 nas_agent_servi、synorelayd；空则显示未知"
          />
        </label>
        <label>
          额外关注端口（可空）
          <input
            value={watchPorts}
            onChange={(e) => setWatchPorts(e.target.value)}
            placeholder="Docker 尚未列出时也认这些端口，例如 8096, 9443"
          />
        </label>
        <label>
          容器显示名
          <textarea
            rows={4}
            value={titleRules}
            onChange={(e) => setTitleRules(e.target.value)}
            placeholder={"每行一条：容器名片段=显示名\njellyfin=Jellyfin"}
          />
        </label>
        <div className="field">
          <span>采集间隔</span>
          <GlideSelect
            value={intervalMs}
            onChange={setIntervalMs}
            options={[
              { value: "1000", label: "1 秒" },
              { value: "2000", label: "2 秒" },
              { value: "5000", label: "5 秒" },
              { value: "10000", label: "10 秒" },
            ]}
          />
        </div>
        <ToggleRow
          label="探测 NAS 出站连通性"
          checked={probe}
          disabled={busy}
          onChange={(next) =>
            void patchFlag(
              { internetProbeEnabled: next },
              next ? "已打开出站探测，将重连 SSH。" : "已关闭出站探测，将重连 SSH。",
            )
          }
        />
        <ToggleRow
          label="开机自启（写入本机启动项，默认关闭）"
          hint={
            autostartOs === false
              ? "系统启动项未登记。开发调试版不能开机自启"
              : undefined
          }
          checked={autostart}
          disabled={busy}
          onChange={(next) =>
            void patchFlag(
              { autostart: next },
              next ? "开机自启已写入本机启动项。" : "已关闭开机自启。",
            )
          }
        />
        <div className="settings-actions">
          <button type="button" className="fit-btn fit-btn-accent" disabled={busy} onClick={save}>
            保存连接
          </button>
          <button type="button" className="fit-btn" disabled={busy} onClick={exportConfig}>
            导出配置
          </button>
          <label className="fit-btn fit-btn-accent">
            导入配置
            <input
              type="file"
              accept="application/json"
              hidden
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) void importFile(file);
                e.currentTarget.value = "";
              }}
            />
          </label>
        </div>
        {message && <p className="muted">{message}</p>}
      </article>

      <article className="card form-card">
        <h2>悬浮窗</h2>
        <ToggleRow
          label="显示悬浮窗"
          checked={overlayOn}
          onChange={(next) => void patchOverlay({ visible: next })}
        />
        <ToggleRow
          label="完全不透明背景"
          hint="开启：深色实底，完全遮住桌面；关闭：半透明玻璃，可透出桌面背景"
          checked={display.opaque}
          onChange={(next) => void patchOverlay({ display: { opaque: next } })}
        />
        <div className="field">
          <span>数值精度</span>
          <GlideSelect
            value={String(display.precision)}
            onChange={(next) => void patchOverlay({ display: { precision: Number(next) } })}
            options={[
              { value: "0", label: "整数，如 CPU 1%" },
              { value: "1", label: "一位小数，如 CPU 1.3%" },
              { value: "2", label: "两位小数，如 CPU 1.32%" },
            ]}
          />
        </div>
        <div className="field-grid">
          {FIELDS.map((field) => (
            <ToggleRow
              key={field.key}
              label={field.label}
              hint={field.hint || undefined}
              checked={Boolean(display[field.key])}
              onChange={(next) => void patchOverlay({ display: { [field.key]: next } })}
            />
          ))}
        </div>
        <button
          type="button"
          className="fit-btn overlay-reset"
          disabled={busy}
          onClick={() => void patchOverlay({ display: { ...defaultOverlayDisplay } })}
        >
          恢复默认显示
        </button>
      </article>
    </div>
  );
}
