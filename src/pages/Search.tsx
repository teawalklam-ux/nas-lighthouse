import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowRight, Search as SearchIcon } from "lucide-react";
import { DriveMounts } from "../components/DriveMounts";
import { bytes } from "../lib/format";
import type { DriveMount, SearchHit, SearchResponse } from "../lib/types";

type SourceFilter = "all" | "local" | "nas" | "quark" | "baidu";

const SOURCE_LABEL: Record<string, string> = {
  local: "本机",
  nas: "NAS",
  quark: "夸克",
  baidu: "百度",
};

const FILTERS: { id: SourceFilter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "local", label: "本机" },
  { id: "nas", label: "NAS" },
  { id: "quark", label: "夸克" },
  { id: "baidu", label: "百度" },
];

export function SearchBar({
  value,
  onChange,
  onSubmit,
  autoFocus,
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit: (value: string) => void;
  autoFocus?: boolean;
}) {
  return (
    <form
      className="dash-search"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(value);
      }}
    >
      <SearchIcon size={18} aria-hidden="true" />
      <input
        type="text"
        value={value}
        autoFocus={autoFocus}
        placeholder="搜索本机、NAS 和网盘"
        onChange={(event) => onChange(event.target.value)}
      />
      <button type="submit" className="dash-search-go" aria-label="搜索">
        <ArrowRight size={18} strokeWidth={2.4} />
      </button>
    </form>
  );
}

export function SearchPage({
  initialQuery,
  drives,
  onDrivesChange,
}: {
  initialQuery: string;
  drives: DriveMount[];
  onDrivesChange: (next: DriveMount[]) => void;
}) {
  const [draft, setDraft] = useState(initialQuery);
  const [query, setQuery] = useState(initialQuery.trim());
  const [filter, setFilter] = useState<SourceFilter>("all");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<SearchResponse | null>(null);

  useEffect(() => {
    setDraft(initialQuery);
    const next = initialQuery.trim();
    if (next) setQuery(next);
  }, [initialQuery]);

  useEffect(() => {
    if (!query) {
      setResult(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setBusy(true);
    setError(null);
    invoke<SearchResponse>("run_search", { query })
      .then((next) => {
        if (!cancelled) setResult(next);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setResult(null);
          setError(err instanceof Error ? err.message : String(err));
        }
      })
      .finally(() => {
        if (!cancelled) setBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [query]);

  const hits = useMemo(() => {
    const all = result?.hits ?? [];
    if (filter === "all") return all;
    return all.filter((hit) => hit.source === filter);
  }, [filter, result]);

  async function openHit(hit: SearchHit) {
    try {
      if (hit.source === "local") {
        await invoke("open_search_hit", { path: hit.path });
        return;
      }
      if (hit.openUrl) await openUrl(hit.openUrl);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function revealHit(hit: SearchHit) {
    try {
      await invoke("reveal_search_hit", { path: hit.path });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div className="page search-page">
      <header className="page-head">
        <div>
          <h1>搜索</h1>
        </div>
      </header>
      <div className="search-sticky">
        <div className="search-sticky-card dash-search-with-dots">
          <SearchBar value={draft} onChange={setDraft} onSubmit={(value) => setQuery(value.trim())} autoFocus />
          <div className="search-toolbar">
            <div className="nav-actions search-filters">
              {FILTERS.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={filter === item.id ? "chip active" : "chip"}
                  onClick={() => setFilter(item.id)}
                >
                  {item.label}
                </button>
              ))}
            </div>
            <DriveMounts mounts={drives} onChange={onDrivesChange} />
          </div>
        </div>
      </div>
      {busy && <p className="muted">正在搜索…</p>}
      {error && <p className="banner bad">{error}</p>}
      {result?.warnings.map((warning) => (
        <p key={warning} className="banner warn">
          {warning}
        </p>
      ))}
      {!busy && query && hits.length === 0 && !error && <p className="muted">没有匹配的文件。</p>}
      <ul className="search-hits">
        {hits.map((hit) => (
          <li key={`${hit.source}:${hit.path}`}>
            <div className="search-hit">
              <button type="button" className="search-hit-main" onClick={() => void openHit(hit)}>
                <span className={`source-tag source-${hit.source}`}>{SOURCE_LABEL[hit.source] ?? hit.source}</span>
                <span className="search-hit-copy">
                  <strong>{hit.name}</strong>
                  <small>{hit.path}</small>
                </span>
                <span className="muted">{hit.size != null ? bytes(hit.size) : hit.isDir ? "文件夹" : ""}</span>
              </button>
              {hit.source === "local" ? (
                <button type="button" className="fit-btn search-hit-folder" onClick={() => void revealHit(hit)}>
                  打开所在文件夹
                </button>
              ) : null}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
