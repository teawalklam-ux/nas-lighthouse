import { useState } from "react";
import { GlideSelect } from "./GlideSelect";
import { NavIcon, ICON_OPTIONS } from "./NavIcon";
import { compactContainerName } from "../lib/format";
import type { ItemHealth, NavItem } from "../lib/types";

export function healthClass(health?: ItemHealth) {
  if (!health || health.state === "unknown") return "tone-unknown";
  if (health.state === "up") return "tone-ok";
  if (health.state === "degraded") return "tone-warn";
  return "tone-bad";
}

export function healthLabel(health?: ItemHealth) {
  if (!health || health.state === "unknown") return "未检测";
  if (health.state === "up") return health.latencyMs != null ? `${health.latencyMs} ms` : "在线";
  const state = health.state === "degraded" ? "需留意" : "离线";
  return health.reason ? `${state} · ${health.reason}` : state;
}

export function NavCard({
  item,
  health,
  busy,
  compact = false,
  onOpen,
  onMove,
  onEdit,
}: {
  item: NavItem;
  health?: ItemHealth;
  busy: boolean;
  compact?: boolean;
  onOpen: () => void;
  onMove: (delta: number) => void;
  onEdit: () => void;
}) {
  const container = item.containerName ? compactContainerName(item.containerName) : null;
  return (
    <article className={`nav-card${compact ? " is-compact" : ""}`} title={item.title}>
      {compact ? (
        <button className="nav-open" type="button" onClick={onOpen}>
          <span className={`dot-wrap ${healthClass(health)}`}>
            <span className="dot" />
          </span>
          <NavIcon name={item.icon} url={item.url} size={28} />
        </button>
      ) : (
        <>
          <div className="nav-card-head">
            <span className={`dot-wrap ${healthClass(health)}`}>
              <span className="dot" />
              {healthLabel(health)}
            </span>
            <button type="button" className="fit-btn" onClick={onOpen} disabled={busy}>
              打开
            </button>
          </div>
          <div className="nav-card-body">
            <NavIcon name={item.icon} url={item.url} size={22} />
            <strong className="title-sm">{item.title}</strong>
            <p title={item.containerName ?? undefined}>
              {item.port}
              {container ? ` · ${container}` : " · 宿主服务"}
            </p>
          </div>
          <div className="nav-actions">
            <button type="button" className="fit-btn" onClick={() => onMove(-1)} disabled={busy}>
              上移
            </button>
            <button type="button" className="fit-btn" onClick={() => onMove(1)} disabled={busy}>
              下移
            </button>
            <button type="button" className="fit-btn" onClick={onEdit} disabled={busy}>
              编辑
            </button>
          </div>
        </>
      )}
    </article>
  );
}

export function EditNavDialog({
  item,
  busy,
  onClose,
  onSave,
  onRemove,
}: {
  item: NavItem;
  busy: boolean;
  onClose: () => void;
  onSave: (item: NavItem) => void;
  onRemove: () => void;
}) {
  const [draft, setDraft] = useState(item);
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <form
        className="card modal"
        onClick={(event) => event.stopPropagation()}
        onSubmit={(event) => {
          event.preventDefault();
          onSave(draft);
        }}
      >
        <h2>编辑导航项</h2>
        <label>
          名称
          <input value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} />
        </label>
        <label>
          地址
          <input value={draft.url} onChange={(e) => setDraft({ ...draft, url: e.target.value })} />
        </label>
        <label>
          分组
          <input value={draft.group} onChange={(e) => setDraft({ ...draft, group: e.target.value })} />
        </label>
        <div className="field">
          <span>图标</span>
          <span className="icon-pick">
            <NavIcon name={draft.icon} url={draft.url} size={28} />
            <GlideSelect
              value={draft.icon}
              onChange={(icon) => setDraft({ ...draft, icon })}
              options={ICON_OPTIONS.map((name) => ({ value: name, label: name }))}
            />
          </span>
          <small>能抓到网站图标时优先显示，抓不到再用预设。</small>
        </div>
        <label className="check">
          <input
            type="checkbox"
            checked={draft.favorite}
            onChange={(e) => setDraft({ ...draft, favorite: e.target.checked })}
          />
          收藏
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={draft.healthCheck}
            onChange={(e) => setDraft({ ...draft, healthCheck: e.target.checked })}
          />
          TCP 健康检查
        </label>
        <div className="nav-actions">
          <button type="submit" className="btn" disabled={busy}>
            保存
          </button>
          <button type="button" onClick={onClose} disabled={busy}>
            取消
          </button>
          <button type="button" className="danger" onClick={onRemove} disabled={busy}>
            删除
          </button>
        </div>
      </form>
    </div>
  );
}
