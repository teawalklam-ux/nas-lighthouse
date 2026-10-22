import type { HealthEvent } from "./types";

export type AlertSummary = {
  warning: number;
  critical: number;
  total: number;
  highest: "warning" | "critical" | null;
};

export function summarizeAlerts(events: HealthEvent[]): AlertSummary {
  let warning = 0;
  let critical = 0;
  for (const event of events) {
    if (event.endedAt !== null) continue;
    if (event.severity === "critical") critical += 1;
    else warning += 1;
  }
  return {
    warning,
    critical,
    total: warning + critical,
    highest: critical > 0 ? "critical" : warning > 0 ? "warning" : null,
  };
}
