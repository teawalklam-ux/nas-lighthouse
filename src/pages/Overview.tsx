import { useEffect, useState } from "react";
import { AlertTriangle, BellOff, CheckCircle2, Clock3, RefreshCw, X } from "lucide-react";
import { DriveMounts } from "../components/DriveMounts";
import { EditNavDialog, NavCard } from "../components/NavCard";
import { StatusDot } from "../components/StatusDot";
import { SearchBar } from "./Search";
import { summarizeAlerts } from "../lib/alerts";
import { ago, bytes, bytesPerSec, compactContainerName, dash, durationBetween, kib, pct, uptime, usedPct } from "../lib/format";
import { internetTitle } from "../lib/status";
import type { DriveMount, HealthEvent, ItemHealth, MetricsSnapshot, NavItem, TempStatus } from "../lib/types";

function tempOf(temps: TempStatus[], label: string): number | null {
  return temps.find((temp) => temp.label === label)?.celsius ?? null;
}

function TempCorner({
  lines,
}: {
  lines: { key: string; label?: string; value: number | null }[];
}) {
  const shown = lines.filter((line) => line.value != null);
  if (shown.length === 0) return null;
  return (
    <div className="card-temps">
      {shown.map((line) => (
        <div key={line.key}>
          {line.label ? <span className="temp-label">{line.label} </span> : null}
          {line.value!.toFixed(1)} °C
        </div>
      ))}
    </div>
  );
}

export function Overview({
  snap,
  navItems,
  health,
  events,
  visibleEvents,
  onDismissAlert,
  onRefresh,
  overviewContainerNames,
  onOpen,
  onMove,
  onUpdate,
  onRemove,
  onSearch,
  drives,
  onDrivesChange,
  configured,
}: {
  snap: MetricsSnapshot;
  navItems: NavItem[];
  health: Record<string, ItemHealth>;
  events: HealthEvent[];
  visibleEvents: HealthEvent[];
  onDismissAlert: (event: HealthEvent, permanently: boolean) => void;
  onRefresh: () => Promise<void>;
  overviewContainerNames: string[];
  configured: boolean;
  onOpen: (url: string) => void;
  onMove: (id: string, delta: number) => Promise<void>;
  onUpdate: (item: NavItem) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
  onSearch: (query: string) => void;
  drives: DriveMount[];
  onDrivesChange: (next: DriveMount[]) => void;
}) {
  const stale = snap.link !== "connected";
  const memUsed =
    snap.mem.totalKb !== null && snap.mem.availableKb !== null
      ? snap.mem.totalKb - snap.mem.availableKb
      : null;
  const memPct =
    memUsed !== null && snap.mem.totalKb
      ? usedPct(memUsed, snap.mem.totalKb)
      : null;
  const raid = snap.raids[0];
  const cpuTemp = tempOf(snap.temps, "CPU");
  const memTemp = tempOf(snap.temps, "内存");
  const nvmeTemp = tempOf(snap.temps, "NVMe");
  const boardTemp = tempOf(snap.temps, "主板");
  const selected = new Set(overviewContainerNames);
  const overviewContainers = snap.containers.filter((c) => selected.has(c.name));
  const cards = [...navItems].sort((a, b) => a.order - b.order);
  const alerts = summarizeAlerts(visibleEvents);
  const activeEvents = visibleEvents.filter((event) => event.endedAt === null);
  const hiddenCount = events.filter((event) => event.endedAt === null).length - activeEvents.length;
  const recentEvents = events.slice(0, 5);
  const waitingForHealth = navItems.some(
    (item) => item.healthCheck && (!health[item.id] || health[item.id].state === "unknown"),
  );
  const summaryTone = alerts.highest
    ? alerts.highest === "critical" ? "bad" : "warn"
    : waitingForHealth ? "unknown" : "ok";
  const summaryText = alerts.total ? `需要关注 · ${alerts.total} 项` : waitingForHealth ? "检测中" : hiddenCount ? "暂无待处理提醒" : "全部正常";

  const [editing, setEditing] = useState<NavItem | null>(null);
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [searchDraft, setSearchDraft] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [notice, setNotice] = useState<{ text: string; autoHide: boolean } | null>(null);

  useEffect(() => {
    if (!notice?.autoHide) return;
    const timer = window.setTimeout(() => setNotice(null), 5_000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  async function refresh() {
    if (refreshing) return;
    setRefreshing(true);
    setLocalError(null);
    try {
      await onRefresh();
      setNotice({ text: "已刷新最新状态", autoHide: true });
    } catch (err) {
      setLocalError(err instanceof Error ? err.message : String(err));
    } finally {
      setRefreshing(false);
    }
  }

  function dismiss(event: HealthEvent, permanently: boolean) {
    try {
      onDismissAlert(event, permanently);
      setLocalError(null);
      setNotice({ text: permanently ? `已关闭「${event.title}」后续提醒` : `已忽略「${event.title}」本次异常，恢复后再次发生时仍会提醒`, autoHide: false });
    } catch {
      setLocalError("无法保存提醒设置，请重试。");
    }
  }

  async function run(work: () => Promise<void>) {
    setBusy(true);
    setLocalError(null);
    try {
      await work();
    } catch (err) {
      setLocalError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={stale ? "page stale" : "page"}>
      <SearchBar value={searchDraft} onChange={setSearchDraft} onSubmit={onSearch} />
      <header className="page-head overview-head">
        <h1>总览</h1>
        <DriveMounts mounts={drives} onChange={onDrivesChange} trailing={
          <button type="button" className={`overview-refresh${refreshing ? " is-refreshing" : ""}`} onClick={() => void refresh()} disabled={refreshing} aria-label={refreshing ? "正在刷新" : "刷新状态"} title={refreshing ? "正在刷新…" : "刷新状态"} aria-busy={refreshing}>
            <RefreshCw size={15} aria-hidden="true" />
          </button>
        }>
          <span
            className={`health-summary tone-${summaryTone}`}
            title={alerts.total ? `${alerts.critical} 项异常，${alerts.warning} 项警告` : hiddenCount ? `已忽略或关闭 ${hiddenCount} 项提醒，可在最近事件中查看` : waitingForHealth ? "正在检测项目状态" : "所有已检测项目状态正常"}
          >
            {alerts.total ? <AlertTriangle size={14} aria-hidden="true" /> : <CheckCircle2 size={14} aria-hidden="true" />}
            <span>{summaryText}</span>
          </span>
          <StatusDot kind="link" state={snap.link} title={stale ? `上次成功 ${ago(snap.lastSuccessTs)}` : "采集流正常"} />
          <StatusDot kind="internet" state={snap.internet.state} title={internetTitle(snap)} />
        </DriveMounts>
      </header>
      {stale ? <p className="muted">数据已暂停更新，上次成功 {ago(snap.lastSuccessTs)}</p> : null}
      {!configured ? <p className="muted">请先到「设置」填写 SSH 别名和 NAS 地址</p> : null}

      {localError && <p className="banner bad">{localError}</p>}
      <p className="overview-feedback" role="status">{notice?.text}</p>

      {activeEvents.length > 0 ? (
        <article className="card attention-card" aria-labelledby="attention-title">
          <div className="section-head">
            <div>
              <h2 id="attention-title">需要关注</h2>
              <p>{alerts.critical ? `${alerts.critical} 项异常` : ""}{alerts.critical && alerts.warning ? " · " : ""}{alerts.warning ? `${alerts.warning} 项警告` : ""}</p>
            </div>
            <AlertTriangle size={20} aria-hidden="true" />
          </div>
          <ul className="issue-list">
            {activeEvents.map((event) => (
              <li key={event.id} className={`issue-row severity-${event.severity}`}>
                <span className="issue-mark" aria-hidden="true" />
                <div className="issue-copy">
                  <strong>{event.title}</strong>
                  <span>{event.message}{event.reason ? ` · ${event.reason}` : ""}</span>
                </div>
                <time dateTime={new Date(event.startedAt).toISOString()}>{durationBetween(event.startedAt)}前开始</time>
                <div className="issue-actions">
                  <button type="button" onClick={() => dismiss(event, false)} title="仅忽略当前这次异常，恢复后再次发生仍会提醒" aria-label={`本次忽略：${event.title}`}><X size={14} aria-hidden="true" />本次忽略</button>
                  <button type="button" onClick={() => dismiss(event, true)} title="关闭该项目今后的提醒，历史事件仍会保留" aria-label={`不再提醒：${event.title}`}><BellOff size={14} aria-hidden="true" />不再提醒</button>
                </div>
              </li>
            ))}
          </ul>
        </article>
      ) : null}

      <section className="cards">
        <article className="card metric">
          <div className="metric-head">
            <h2>CPU</h2>
            <TempCorner lines={[{ key: "cpu", value: cpuTemp }]} />
          </div>
          <div className="metric-body">
            <strong>{dash(snap.cpu.usagePct, pct)}</strong>
            <p>
              负载 {dash(snap.cpu.load1, (n) => n.toFixed(2))}
              {snap.cpu.cores ? ` · ${snap.cpu.cores} 核` : ""}
            </p>
          </div>
        </article>
        <article className="card metric">
          <div className="metric-head">
            <h2>内存</h2>
            <TempCorner
              lines={[
                { key: "mem", label: "内存", value: memTemp },
                { key: "nvme", label: "NVMe", value: nvmeTemp },
              ]}
            />
          </div>
          <div className="metric-body">
            <strong>{dash(memPct, pct)}</strong>
            <p>
              {dash(snap.mem.availableKb, kib)} 可用 / {dash(snap.mem.totalKb, kib)}
            </p>
          </div>
        </article>
        <article className="card metric">
          <div className="metric-head">
            <h2>网络{snap.net.iface ? ` ${snap.net.iface}` : ""}</h2>
          </div>
          <div className="metric-body">
            <strong>↓ {dash(snap.net.rxBytesPerSec, bytesPerSec)}</strong>
            <p>↑ {dash(snap.net.txBytesPerSec, bytesPerSec)}</p>
          </div>
        </article>
        <article className="card metric">
          <div className="metric-head">
            <h2>运行时长</h2>
            <TempCorner lines={[{ key: "board", label: "主板", value: boardTemp }]} />
          </div>
          <div className="metric-body">
            <strong>{dash(snap.uptimeSec, uptime)}</strong>
            <p>系统持续运行时间</p>
          </div>
        </article>
      </section>

      <article className="card storage-card">
        <h2>存储</h2>
        {snap.disks.length === 0 ? (
          <p className="muted">—</p>
        ) : (
          <ul className="bars">
            {snap.disks.map((disk) => {
              const p = usedPct(disk.usedBytes, disk.totalBytes);
              return (
                <li key={disk.mount}>
                  <div className="bar-label">
                    <span>{disk.mount}</span>
                    <span>
                      {dash(p, pct)} · {bytes(disk.usedBytes)} / {bytes(disk.totalBytes)}
                    </span>
                  </div>
                  <div className="bar">
                    <span style={{ width: `${Math.min(p ?? 0, 100)}%` }} />
                  </div>
                </li>
              );
            })}
          </ul>
        )}
        {raid ? (
          <p className={raid.healthy ? "raid ok" : "raid bad"}>
            {raid.name} {raid.level} · {raid.disksUp}/{raid.disksTotal} [{raid.devices}]
            {raid.healthy ? " · 健康" : raid.recovering ? ` · 重建 ${dash(raid.recoveryPct, pct)}` : " · 异常"}
          </p>
        ) : (
          <p className="muted">阵列状态 —</p>
        )}
      </article>

      {cards.length > 0 && (
        <section className="nav-grid overview-nav">
          {cards.map((item) => (
            <NavCard
              key={item.id}
              item={item}
              health={health[item.id]}
              busy={busy}
              onOpen={() => onOpen(item.url)}
              onMove={(delta) => run(() => onMove(item.id, delta))}
              onEdit={() => setEditing(item)}
            />
          ))}
        </section>
      )}

      <article className="card">
        <h2>容器</h2>
        {overviewContainerNames.length === 0 ? (
          <p className="muted">尚未挑选。到「容器」页勾选要出现在总览的容器。</p>
        ) : overviewContainers.length === 0 ? (
          <p className="muted">已勾选的容器当前不在运行列表中。</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>名称</th>
                <th>状态</th>
                <th>健康</th>
                <th>端口</th>
              </tr>
            </thead>
            <tbody>
              {overviewContainers.map((c) => (
                <tr key={c.name}>
                  <td title={c.name}>{compactContainerName(c.name)}</td>
                  <td>{c.state}</td>
                  <td>{c.health ?? "—"}</td>
                  <td>
                    {c.ports.length
                      ? c.ports.map((p) => `${p.host}→${p.container}/${p.proto}`).join(", ")
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </article>

      {recentEvents.length > 0 ? (
        <article className="card event-card" aria-labelledby="event-title">
          <div className="section-head">
            <div>
              <h2 id="event-title">最近事件</h2>
              <p>仅记录状态变化，最多保留 300 条</p>
            </div>
            <Clock3 size={19} aria-hidden="true" />
          </div>
          <ol className="event-list">
            {recentEvents.map((event) => (
              <li key={event.id}>
                <span className={`event-state ${event.endedAt === null ? `severity-${event.severity}` : "is-recovered"}`}>
                  {event.endedAt === null ? (event.severity === "critical" ? "异常" : "警告") : "已恢复"}
                </span>
                <div>
                  <strong>{event.title}</strong>
                  <p>{event.message}{event.reason ? ` · ${event.reason}` : ""}</p>
                </div>
                <time dateTime={new Date(event.startedAt).toISOString()}>
                  {event.endedAt === null
                    ? ago(event.startedAt)
                    : `${ago(event.endedAt)} · 持续 ${durationBetween(event.startedAt, event.endedAt)}`}
                </time>
              </li>
            ))}
          </ol>
        </article>
      ) : null}

      {editing && (
        <EditNavDialog
          key={editing.id}
          item={editing}
          busy={busy}
          onClose={() => setEditing(null)}
          onSave={(item) =>
            void run(async () => {
              await onUpdate(item);
              setEditing(null);
            })
          }
          onRemove={() =>
            void run(async () => {
              await onRemove(editing.id);
              setEditing(null);
            })
          }
        />
      )}
    </div>
  );
}
