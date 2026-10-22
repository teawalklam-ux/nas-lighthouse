import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { alertSourceKey, isAlertVisible, readAlertPreferences, saveAlertPreferences } from "./alertPreferences";
import type { AppConfig, Candidate, DriveMount, HealthEvent, ItemHealth, MetricsSnapshot, NavItem } from "./types";

export function useWorkspace() {
  const [snap, setSnap] = useState<MetricsSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [health, setHealth] = useState<Record<string, ItemHealth>>({});
  const [events, setEvents] = useState<HealthEvent[]>([]);
  const [drives, setDrives] = useState<DriveMount[]>([]);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [alertPreferences, setAlertPreferences] = useState(readAlertPreferences);

  function dismissAlert(event: HealthEvent, permanently: boolean) {
    const next = {
      ignored: alertPreferences.ignored.filter((id) => events.some((item) => item.id === id && item.endedAt === null)),
      muted: [...alertPreferences.muted],
    };
    if (permanently) next.muted = [...new Set([...next.muted, alertSourceKey(event)])];
    else next.ignored = [...new Set([...next.ignored, event.id])];
    saveAlertPreferences(next);
    setAlertPreferences(next);
  }

  async function refreshOverview() {
    const results = await Promise.allSettled([
      invoke<MetricsSnapshot>("get_snapshot").then(setSnap),
      invoke<Record<string, ItemHealth>>("get_health").then(setHealth),
      invoke<DriveMount[]>("refresh_drive_mounts").then(setDrives),
    ]);
    // Mount checks may produce new events; fetch these after all checks finish.
    await invoke<HealthEvent[]>("get_events").then(setEvents);
    const failure = results.find((result) => result.status === "rejected");
    if (failure?.status === "rejected") throw new Error(`刷新未完成：${String(failure.reason)}`);
  }

  const refreshConfig = useCallback(async () => {
    const next = await invoke<AppConfig>("get_config");
    setConfig(next);
    return next;
  }, []);

  const refreshCandidates = useCallback(async () => {
    const next = await invoke<Candidate[]>("list_candidates");
    setCandidates(next);
  }, []);

  useEffect(() => {
    const unlistenMetrics = listen<MetricsSnapshot>("metrics", (event) => setSnap(event.payload));
    const unlistenHealth = listen<Record<string, ItemHealth>>("health", (event) => setHealth(event.payload));
    const unlistenEvents = listen<HealthEvent[]>("events", (event) => setEvents(event.payload));
    const unlistenDrives = listen<DriveMount[]>("drives", (event) => setDrives(event.payload));
    const unlistenConfig = listen<AppConfig>("config", (event) => setConfig(event.payload));
    invoke<MetricsSnapshot>("get_snapshot").then(setSnap).catch(() => setSnap(null));
    invoke<Record<string, ItemHealth>>("get_health").then(setHealth).catch(() => undefined);
    invoke<HealthEvent[]>("get_events").then(setEvents).catch(() => undefined);
    invoke<DriveMount[]>("get_drive_mounts").then(setDrives).catch(() => undefined);
    refreshConfig().catch((err: Error) => setError(String(err)));
    return () => {
      void unlistenMetrics.then((fn) => fn());
      void unlistenHealth.then((fn) => fn());
      void unlistenEvents.then((fn) => fn());
      void unlistenDrives.then((fn) => fn());
      void unlistenConfig.then((fn) => fn());
    };
  }, [refreshConfig]);

  useEffect(() => {
    refreshCandidates().catch(() => undefined);
  }, [snap, config, refreshCandidates]);

  const applyConfig = useCallback(
    async (next: AppConfig) => {
      setConfig(next);
      setError(null);
      await refreshCandidates();
    },
    [refreshCandidates],
  );

  const addItem = useCallback(
    async (candidate: Candidate) => {
      const next = await invoke<AppConfig>("add_nav_item", { candidate });
      await applyConfig(next);
    },
    [applyConfig],
  );

  const updateItem = useCallback(
    async (item: NavItem) => {
      const next = await invoke<AppConfig>("update_nav_item", { item });
      await applyConfig(next);
    },
    [applyConfig],
  );

  const removeItem = useCallback(
    async (id: string) => {
      const next = await invoke<AppConfig>("remove_nav_item", { id });
      await applyConfig(next);
    },
    [applyConfig],
  );

  const moveItem = useCallback(
    async (id: string, delta: number) => {
      const next = await invoke<AppConfig>("move_nav_item", { id, delta });
      await applyConfig(next);
    },
    [applyConfig],
  );

  const setOverviewContainer = useCallback(
    async (name: string, visible: boolean) => {
      const next = await invoke<AppConfig>("set_overview_container", { name, visible });
      await applyConfig(next);
    },
    [applyConfig],
  );

  const setOverviewContainers = useCallback(
    async (names: string[]) => {
      const next = await invoke<AppConfig>("set_overview_containers", { names });
      await applyConfig(next);
    },
    [applyConfig],
  );

  return {
    snap,
    config,
    health,
    events,
    visibleEvents: events.filter((event) => isAlertVisible(event, alertPreferences)),
    dismissAlert,
    refreshOverview,
    drives,
    setDrives,
    candidates,
    error,
    setError,
    setConfig: applyConfig,
    refreshConfig,
    addItem,
    updateItem,
    removeItem,
    moveItem,
    setOverviewContainer,
    setOverviewContainers,
  };
}
