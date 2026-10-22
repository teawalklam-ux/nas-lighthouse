import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { LucideIcon } from "lucide-react";
import { EyeOff, LayoutDashboard, Power, Search, Settings } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { SearchBar } from "./pages/Search";
import { bytes } from "./lib/format";
import type { SearchHit, SearchResponse } from "./lib/types";
import "./styles.css";
import "./overlay-pop.css";

type PopKind = "menu" | "search";

type PopEvent = {
  kind: PopKind;
  token: number;
};

const SOURCE_LABEL: Record<string, string> = {
  local: "本机",
  nas: "NAS",
  quark: "夸克",
  baidu: "百度",
};

const ROWS: { id: string; label: string; icon: LucideIcon; danger?: boolean }[] = [
  { id: "main", label: "打开主面板", icon: LayoutDashboard },
  { id: "search", label: "打开搜索框", icon: Search },
  { id: "hide", label: "隐藏悬浮球", icon: EyeOff },
  { id: "settings", label: "设置", icon: Settings },
  { id: "quit", label: "退出", icon: Power, danger: true },
];

function reducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function OverlayPop() {
  const shellRef = useRef<HTMLDivElement>(null);
  const [kind, setKind] = useState<PopKind>("menu");
  const [token, setToken] = useState(0);
  const [entered, setEntered] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [cursor, setCursor] = useState({ index: 0, top: 0, visible: false });
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hits, setHits] = useState<SearchHit[]>([]);

  const allowClose = useRef(false);

  function apply(event: PopEvent) {
    setKind(event.kind === "search" ? "search" : "menu");
    setToken(event.token);
    setLeaving(false);
    setCursor({ index: 0, top: 0, visible: false });
    if (event.kind !== "search") {
      setQuery("");
      setHits([]);
      setError(null);
    }
  }

  useEffect(() => {
    invoke<PopEvent>("overlay_pop_snapshot").then(apply).catch(() => undefined);
    const unlisten = listen<PopEvent>("overlay-pop", (event) => apply(event.payload));
    const focus = getCurrentWindow().listen("tauri://focus", () => {
      window.setTimeout(() => {
        allowClose.current = true;
      }, 500);
    });
    const blur = getCurrentWindow().listen("tauri://blur", () => {
      if (allowClose.current) void invoke("hide_overlay_pop");
    });
    return () => {
      void unlisten.then((fn) => fn());
      void focus.then((fn) => fn());
      void blur.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    allowClose.current = false;
  }, [token]);

  useEffect(() => {
    if (reducedMotion()) {
      setEntered(true);
      return;
    }
    setEntered(false);
    const frame = requestAnimationFrame(() => setEntered(true));
    return () => cancelAnimationFrame(frame);
  }, [token, kind]);

  useEffect(() => {
    if (kind !== "search" || !query) return;
    let cancelled = false;
    setBusy(true);
    setError(null);
    invoke<SearchResponse>("run_search", { query })
      .then((result) => {
        if (cancelled) return;
        setHits(result.hits.slice(0, 8));
        setError(result.warnings[0] ?? null);
      })
      .catch((err: unknown) => {
        if (!cancelled) setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [kind, query]);

  useLayoutEffect(() => {
    const shell = shellRef.current;
    if (!shell || kind !== "search") return;
    const height = Math.ceil(shell.scrollHeight);
    if (height < 96) return;
    void invoke("resize_overlay_pop", { height });
  }, [kind, hits, busy, error, token]);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        void invoke("hide_overlay_pop");
        return;
      }
      if (kind !== "menu") return;
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        const delta = event.key === "ArrowDown" ? 1 : -1;
        const index = (cursor.index + delta + ROWS.length) % ROWS.length;
        const button = shellRef.current?.querySelectorAll<HTMLButtonElement>(".glide-row")[index];
        setCursor({ index, top: Math.max(0, (button?.offsetTop ?? 6) - 6), visible: true });
      }
      if (event.key === "Enter") {
        event.preventDefault();
        void choose(ROWS[cursor.index]?.id ?? "main");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function choose(id: string) {
    if (id === "search") {
      setLeaving(true);
      const pos = await getCurrentWindow().outerPosition();
      window.setTimeout(() => {
        void invoke("open_overlay_pop", { kind: "search", x: pos.x + 24, y: pos.y + 24 }).catch(() => undefined);
      }, reducedMotion() ? 0 : 180);
      return;
    }
    if (id === "main") await invoke("open_main_page", { page: "overview" });
    if (id === "settings") await invoke("open_main_page", { page: "settings" });
    if (id === "hide") await invoke("set_overlay_state", { patch: { visible: false } });
    if (id === "quit") {
      await invoke("quit_app");
      return;
    }
    await invoke("hide_overlay_pop");
  }

  async function openHit(hit: SearchHit) {
    if (hit.source === "local") {
      await invoke("open_search_hit", { path: hit.path });
      return;
    }
    if (hit.openUrl) await openUrl(hit.openUrl);
  }

  const surfaceClass = `glide-surface${entered ? " is-in" : ""}${leaving ? " is-out" : ""}`;

  return (
    <div ref={shellRef} className={`pop-shell${kind === "search" ? " is-search" : ""}`}>
      {kind === "menu" ? (
        <div className={surfaceClass} role="menu" onMouseLeave={() => setCursor((state) => ({ ...state, visible: false }))}>
          <span
            className={`glide-highlight${cursor.visible ? " is-on" : ""}`}
            style={{ transform: `translateY(${cursor.top}px)` }}
            aria-hidden="true"
          />
          {ROWS.map((row, index) => (
            <span key={row.id} className="glide-row-wrap">
              {row.id === "quit" ? <span className="glide-rule" /> : null}
              <button
                type="button"
                role="menuitem"
                className={`glide-row${row.danger ? " is-danger" : ""}`}
                onMouseEnter={(event) => setCursor({ index, top: Math.max(0, event.currentTarget.offsetTop - 6), visible: true })}
                onClick={() => void choose(row.id)}
              >
                <row.icon size={14} strokeWidth={1.7} aria-hidden="true" />
                <span>{row.label}</span>
              </button>
            </span>
          ))}
        </div>
      ) : (
        <div className={`float-search-card ${surfaceClass}`}>
          <SearchBar value={draft} onChange={setDraft} onSubmit={(value) => setQuery(value.trim())} autoFocus />
          {busy ? <p className="float-note">正在搜索…</p> : null}
          {error ? <p className="float-note is-warn">{error}</p> : null}
          {hits.length > 0 ? (
            <ul className="float-hits">
              {hits.map((hit) => (
                <li key={`${hit.source}:${hit.path}`}>
                  <button type="button" className="float-hit" onClick={() => void openHit(hit)}>
                    <span className={`source-tag source-${hit.source}`}>{SOURCE_LABEL[hit.source] ?? hit.source}</span>
                    <span className="float-hit-copy">
                      <strong>{hit.name}</strong>
                      <small>{hit.path}</small>
                    </span>
                    <span className="muted">{hit.size != null ? bytes(hit.size) : hit.isDir ? "文件夹" : ""}</span>
                  </button>
                </li>
              ))}
            </ul>
          ) : null}
          {!busy && query && hits.length === 0 && !error ? <p className="float-note">没有匹配的文件。</p> : null}
        </div>
      )}
    </div>
  );
}
