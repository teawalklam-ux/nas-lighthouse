import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Box,
  Compass,
  LayoutDashboard,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Settings,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Overview } from "./pages/Overview";
import { Containers } from "./pages/Containers";
import { NavPage } from "./pages/Nav";
import { SearchPage } from "./pages/Search";
import { SettingsPage } from "./pages/Settings";
import { StatusBloub } from "./components/StatusBloub";
import { parseBloubPin } from "./lib/bloubMood";
import { Titlebar } from "./components/Titlebar";
import { useWorkspace } from "./lib/useWorkspace";
import { summarizeAlerts } from "./lib/alerts";
import type { AppConfig } from "./lib/types";
import "./styles.css";

type Page = "overview" | "search" | "nav" | "containers" | "settings";

export default function App() {
  const workspace = useWorkspace();
  const [page, setPage] = useState<Page>("overview");
  const [searchQuery, setSearchQuery] = useState("");
  const snap = workspace.snap;
  const sidebarCollapsed = Boolean(workspace.config?.ui.sidebarCollapsed);
  const alertSummary = summarizeAlerts(workspace.visibleEvents);

  useEffect(() => {
    const unlisten = listen<{ page?: string; query?: string | null }>("navigate", (event) => {
      const next = event.payload.page;
      if (next === "overview" || next === "search" || next === "nav" || next === "containers" || next === "settings") {
        setPage(next);
      }
      if (typeof event.payload.query === "string") setSearchQuery(event.payload.query);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  function goSearch(query: string) {
    const next = query.trim();
    if (!next) return;
    setSearchQuery(next);
    setPage("search");
  }

  async function setSidebarCollapsed(next: boolean) {
    const saved = await invoke<AppConfig>("save_settings", { patch: { sidebarCollapsed: next } });
    await workspace.setConfig(saved);
  }

  return (
    <div className={`shell${sidebarCollapsed ? " is-slim" : ""}`}>
      <Titlebar />
      <aside>
        <nav>
          <button
            className={page === "overview" ? "active" : ""}
            onClick={() => setPage("overview")}
            title="总览"
          >
            <LayoutDashboard size={18} />
            <span className="sidebar-label" aria-hidden={sidebarCollapsed}>总览</span>
          </button>
          <button className={page === "search" ? "active" : ""} onClick={() => setPage("search")} title="搜索">
            <Search size={18} />
            <span className="sidebar-label" aria-hidden={sidebarCollapsed}>搜索</span>
          </button>
          <button className={page === "nav" ? "active" : ""} onClick={() => setPage("nav")} title="导航">
            <Compass size={18} />
            <span className="sidebar-label" aria-hidden={sidebarCollapsed}>导航</span>
          </button>
          <button
            className={page === "containers" ? "active" : ""}
            onClick={() => setPage("containers")}
            title="容器"
          >
            <Box size={18} />
            <span className="sidebar-label" aria-hidden={sidebarCollapsed}>容器</span>
          </button>
          <button
            className={page === "settings" ? "active" : ""}
            onClick={() => setPage("settings")}
            title="设置"
          >
            <Settings size={18} />
            <span className="sidebar-label" aria-hidden={sidebarCollapsed}>设置</span>
          </button>
        </nav>
        <button
          className="sidebar-toggle"
          type="button"
          title={sidebarCollapsed ? "展开导航" : "收起"}
          onClick={() => void setSidebarCollapsed(!sidebarCollapsed)}
        >
          {sidebarCollapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
          <span className="sidebar-label" aria-hidden={sidebarCollapsed}>收起</span>
        </button>
      </aside>
      <main>
        {!snap || !workspace.config ? (
          <div className="page">
            <p className="muted">正在启动采集…</p>
          </div>
        ) : (
          <>
            {page === "overview" && (
              <Overview
                snap={snap}
                navItems={workspace.config.navItems}
                health={workspace.health}
                events={workspace.events}
                visibleEvents={workspace.visibleEvents}
                onDismissAlert={workspace.dismissAlert}
                onRefresh={workspace.refreshOverview}
                overviewContainerNames={workspace.config.overviewContainerNames ?? []}
                configured={Boolean(workspace.config.sshAlias && workspace.config.nasHost)}
                onOpen={(url) => openUrl(url)}
                onMove={workspace.moveItem}
                onUpdate={workspace.updateItem}
                onRemove={workspace.removeItem}
                onSearch={goSearch}
                drives={workspace.drives}
                onDrivesChange={workspace.setDrives}
              />
            )}
            {page === "search" && (
              <SearchPage
                initialQuery={searchQuery}
                drives={workspace.drives}
                onDrivesChange={workspace.setDrives}
              />
            )}
            {page === "containers" && (
              <Containers
                snap={snap}
                overviewContainerNames={workspace.config.overviewContainerNames ?? []}
                onToggleOverview={workspace.setOverviewContainer}
                onSetOverviewContainers={workspace.setOverviewContainers}
              />
            )}
            {page === "nav" && (
              <NavPage
                config={workspace.config}
                candidates={workspace.candidates}
                health={workspace.health}
                error={workspace.error}
                onAdd={workspace.addItem}
                onUpdate={workspace.updateItem}
                onRemove={workspace.removeItem}
                onMove={workspace.moveItem}
                onToggleCompact={async (compact) => {
                  const saved = await invoke<AppConfig>("save_settings", { patch: { navCompact: compact } });
                  await workspace.setConfig(saved);
                }}
              />
            )}
            {page === "settings" && (
              <SettingsPage config={workspace.config} onSave={workspace.setConfig} />
            )}
          </>
        )}
      </main>
      <StatusBloub
        snap={snap}
        health={workspace.health}
        alerts={alertSummary}
        pin={parseBloubPin(workspace.config?.ui.bloubPin)}
      />
    </div>
  );
}
