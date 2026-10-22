import { useState } from "react";
import { compactContainerName } from "../lib/format";
import type { MetricsSnapshot } from "../lib/types";

export function Containers({
  snap,
  overviewContainerNames,
  onToggleOverview,
  onSetOverviewContainers,
}: {
  snap: MetricsSnapshot;
  overviewContainerNames: string[];
  onToggleOverview: (name: string, visible: boolean) => Promise<void>;
  onSetOverviewContainers: (names: string[]) => Promise<void>;
}) {
  const selected = new Set(overviewContainerNames);
  const names = snap.containers.map((c) => c.name);
  const allSelected = names.length > 0 && names.every((name) => selected.has(name));
  const noneSelected = overviewContainerNames.length === 0;
  const [busy, setBusy] = useState<"all" | "none" | null>(null);

  async function run(kind: "all" | "none", namesToSet: string[]) {
    setBusy(kind);
    try {
      await onSetOverviewContainers(namesToSet);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>容器</h1>
        </div>
        {names.length > 0 ? (
          <div className="nav-actions">
            <button
              type="button"
              disabled={busy !== null || allSelected}
              onClick={() => void run("all", names)}
            >
              全选
            </button>
            <button
              type="button"
              disabled={busy !== null || noneSelected}
              onClick={() => void run("none", [])}
            >
              全不选
            </button>
          </div>
        ) : null}
      </header>
      {snap.containers.length === 0 ? (
        <p className="muted">—</p>
      ) : (
        <article className="card">
          <table>
            <thead>
              <tr>
                <th className="check-cell">总览</th>
                <th>名称</th>
                <th>镜像</th>
                <th>状态</th>
                <th>健康</th>
                <th>端口</th>
              </tr>
            </thead>
            <tbody>
              {snap.containers.map((c) => (
                <tr key={c.name}>
                  <td className="check-cell">
                    <label className="check table-check">
                      <input
                        type="checkbox"
                        checked={selected.has(c.name)}
                        onChange={(e) => void onToggleOverview(c.name, e.target.checked)}
                      />
                      <span className="sr-only">在总览显示 {c.name}</span>
                    </label>
                  </td>
                  <td title={c.name}>{compactContainerName(c.name)}</td>
                  <td>{c.image}</td>
                  <td>{c.status}</td>
                  <td>{c.health ?? "—"}</td>
                  <td>
                    {c.ports.length
                      ? c.ports.map((p) => `${p.host}/${p.proto}`).join(", ")
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </article>
      )}
    </div>
  );
}
