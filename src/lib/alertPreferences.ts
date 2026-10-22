import type { HealthEvent } from "./types";

const STORAGE_KEY = "nas-lighthouse.alert-preferences.v1";
export type AlertPreferences = { ignored: string[]; muted: string[] };

export function readAlertPreferences(): AlertPreferences {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    return {
      ignored: Array.isArray(value?.ignored) ? value.ignored.filter((id: unknown) => typeof id === "string") : [],
      muted: Array.isArray(value?.muted) ? value.muted.filter((id: unknown) => typeof id === "string") : [],
    };
  } catch {
    return { ignored: [], muted: [] };
  }
}

export function alertSourceKey(event: HealthEvent): string {
  return JSON.stringify([event.source, event.sourceId]);
}

export function isAlertVisible(event: HealthEvent, preferences: AlertPreferences): boolean {
  return !preferences.ignored.includes(event.id) && !preferences.muted.includes(alertSourceKey(event));
}

export function saveAlertPreferences(preferences: AlertPreferences): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
}
