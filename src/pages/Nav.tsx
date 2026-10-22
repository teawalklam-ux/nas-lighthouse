import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { EditNavDialog, NavCard } from "../components/NavCard";
import { NavIcon } from "../components/NavIcon";
import { compactContainerName } from "../lib/format";
import type { AppConfig, Candidate, ItemHealth, NavItem } from "../lib/types";

type Filter = "all" | "favorite" | string;

export function NavPage({
  config,
  candidates,
  health,
  error,
  onAdd,
  onUpdate,
  onRemove,
  onMove,
  onToggleCompact,
}: {
  config: AppConfig;
  candidates: Candidate[];
  health: Record<string, ItemHealth>;
  error: string | null;
  onAdd: (candidate: Candidate) => Promise<void>;
  onUpdate: (item: NavItem) => Promise<void>;
  onRemove: (id: string) => Promise<void>;
  onMove: (id: string, delta: number) => Promise<void>;
  onToggleCompact: (compact: boolean) => Promise<void>;
}) {
  const [filter, setFilter] = useState<Filter>("all");
  const [editing, setEditing] = useState<NavItem | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);

  const groups = useMemo(() => {
    const set = new Set(config.navItems.map((item) => item.group || "NAS 服务"));
    return [...set];
  }, [config.navItems]);

  const items = useMemo(() => {
    const sorted = [...config.navItems].sort((a, b) => a.order - b.order);
    if (filter === "favorite") return sorted.filter((item) => item.favorite);
    if (filter !== "all") return sorted.filter((item) => item.group === filter);
    return sorted;
  }, [config.navItems, filter]);

  async function open(item: NavItem) {
    await openUrl(item.url);
  }

  async function run(label: string, work: () => Promise<void>) {
    setBusy(label);
    setLocalError(null);
    try {
      await work();
    } catch (err) {
      setLocalError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>导航</h1>
          <p>从容器端口和本机访问记录收录服务。</p>
        </div>
        <div className="nav-actions">
          <button
            type="button"
            className={config.ui.navCompact ? "chip active" : "chip"}
            onClick={() => void onToggleCompact(!config.ui.navCompact)}
          >
            {config.ui.navCompact ? "展开卡片" : "仅图标"}
          </button>
        </div>
      </header>

      {(error || localError) && <p className="banner bad">{error || localError}</p>}

      {candidates.length > 0 && (
        <article className="card">
          <h2>发现未收录的服务</h2>
          <ul className="candidate-list">
            {candidates.map((candidate) => (
              <li key={`${candidate.host}:${candidate.port}`}>
                <div className="candidate-copy">
                  <NavIcon name="box" url={candidate.url} size={22} />
                  <div>
                    <strong className="title-sm">{candidate.suggestedTitle}</strong>
                    <p>
                      {candidate.host}:{candidate.port}
                      {candidate.containerName
                        ? ` · ${compactContainerName(candidate.containerName)}`
                        : " · 宿主服务"}
                      {candidate.source === "access" ? " · 正在访问" : ""}
                    </p>
                  </div>
                </div>
                <button
                  type="button"
                  className="fit-btn"
                  disabled={busy !== null}
                  onClick={() => run(`add-${candidate.port}`, () => onAdd(candidate))}
                >
                  收录
                </button>
              </li>
            ))}
          </ul>
        </article>
      )}

      <div className="chips">
        <button className={filter === "all" ? "chip active" : "chip"} onClick={() => setFilter("all")}>
          全部
        </button>
        <button className={filter === "favorite" ? "chip active" : "chip"} onClick={() => setFilter("favorite")}>
          收藏
        </button>
        {groups.map((group) => (
          <button
            key={group}
            className={filter === group ? "chip active" : "chip"}
            onClick={() => setFilter(group)}
          >
            {group}
          </button>
        ))}
      </div>

      {items.length === 0 ? (
        <p className="muted">还没有导航项。上方发现列表可以一键收录 NAS 上已映射端口的服务。</p>
      ) : (
        <section className={`nav-grid${config.ui.navCompact ? " is-compact" : ""}`}>
          {items.map((item) => (
            <NavCard
              key={item.id}
              item={item}
              health={health[item.id]}
              busy={busy !== null}
              compact={config.ui.navCompact}
              onOpen={() => open(item)}
              onMove={(delta) => run(`move-${item.id}`, () => onMove(item.id, delta))}
              onEdit={() => setEditing(item)}
            />
          ))}
        </section>
      )}

      {editing && (
        <EditNavDialog
          key={editing.id}
          item={editing}
          busy={busy !== null}
          onClose={() => setEditing(null)}
          onSave={(item) =>
            run("save", async () => {
              await onUpdate(item);
              setEditing(null);
            })
          }
          onRemove={() =>
            run("remove", async () => {
              await onRemove(editing.id);
              setEditing(null);
            })
          }
        />
      )}
    </div>
  );
}
