import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BloubCanvas } from "./BloubCanvas";
import {
  BLOUB_CHOICES,
  bloubPaper,
  parseBloubPin,
  resolveBloub,
  type BloubPin,
} from "../lib/bloubMood";
import { STATE_BY_ID } from "../vendor/bloub/states";
import type { AppConfig, ItemHealth, MetricsSnapshot } from "../lib/types";
import type { AlertSummary } from "../lib/alerts";

const OPEN_MS = Math.round((STATE_BY_ID.get("orbit")?.duration ?? 3.4) * 1000);

export function StatusBloub({
  snap,
  health,
  alerts,
  pin: pinProp = "auto",
  variant = "page",
  live = true,
  onActivate,
}: {
  snap: MetricsSnapshot | null;
  health: Record<string, ItemHealth>;
  alerts?: AlertSummary;
  pin?: BloubPin;
  variant?: "page" | "overlay";
  live?: boolean;
  onActivate?: () => void;
}) {  const [open, setOpen] = useState(false);
  const [hover, setHover] = useState(false);
  const [opening, setOpening] = useState(false);
  const [pendingPin, setPendingPin] = useState<BloubPin | null>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const openTimer = useRef(0);
  const pin = pendingPin ?? parseBloubPin(pinProp);
  const mood = resolveBloub(pin, snap, health, variant, { hover, opening }, alerts);
  const size = variant === "overlay" ? 47 : 64;
  const picker = variant === "page";

  const playOpen = useCallback(() => {
    setOpening(true);
    window.clearTimeout(openTimer.current);
    openTimer.current = window.setTimeout(() => setOpening(false), OPEN_MS);
  }, []);

  useEffect(() => {
    setPendingPin(null);
  }, [pinProp]);

  useEffect(() => {
    if (variant === "page") playOpen();
    const pending = listen("main-shown", () => playOpen()).catch(() => undefined);
    return () => {
      window.clearTimeout(openTimer.current);
      void pending.then((fn) => fn?.());
    };
  }, [playOpen, variant]);

  useEffect(() => {
    if (!open || !picker) return;
    const onPointer = (event: PointerEvent) => {
      if (!wrapRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [open, picker]);

  const title = picker ? `${mood.reason} · 点击选择动画` : `${mood.reason} · 点击打开主窗口`;

  return (
    <div ref={wrapRef} className={`bloub-dock bloub-${variant}${open ? " is-open" : ""}`}>
      <button
        type="button"
        className="bloub-hit no-drag"
        style={{ ["--bloub-ink" as string]: mood.ink }}
        aria-haspopup={picker ? "menu" : undefined}
        aria-expanded={picker ? open : undefined}
        title={title}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onClick={(event) => {
          event.stopPropagation();
          if (picker) {
            setOpen((value) => !value);
            return;
          }
          playOpen();
          onActivate?.();
        }}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <BloubCanvas
          state={mood.state}
          size={size}
          ink={mood.ink}
          paper={bloubPaper(variant)}
          live={live}
          followPointer={variant === "overlay" && mood.state === "idle"}
        />
        {variant === "overlay" && alerts && alerts.total > 0 ? (
          <span
            className={`bloub-alert-badge ${alerts.critical ? "is-critical" : "is-warning"}`}
            aria-hidden="true"
          >
            {Math.min(alerts.total, 99)}
          </span>
        ) : null}
      </button>
      {picker && open ? (
        <div className="bloub-pop no-drag" role="menu" onMouseDown={(event) => event.stopPropagation()}>
          <p className="bloub-pop-status">{mood.reason}</p>
          {BLOUB_CHOICES.map((choice) => (
            <button
              key={choice.id}
              type="button"
              role="menuitemradio"
              aria-checked={pin === choice.id}
              className={pin === choice.id ? "is-active" : ""}
              onClick={() => {
                setOpen(false);
                setPendingPin(choice.id);
                void invoke<AppConfig>("save_settings", { patch: { bloubPin: choice.id } });
              }}
            >
              <strong>{choice.label}</strong>
              <span>{choice.hint}</span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
