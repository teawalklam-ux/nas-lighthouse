import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MetricsSnapshot } from "./types";

export function useMetrics(): MetricsSnapshot | null {
  const [snap, setSnap] = useState<MetricsSnapshot | null>(null);

  useEffect(() => {
    const unlisten = listen<MetricsSnapshot>("metrics", (event) => {
      setSnap(event.payload);
    });
    invoke<MetricsSnapshot>("get_snapshot")
      .then(setSnap)
      .catch(() => setSnap(null));
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  return snap;
}
