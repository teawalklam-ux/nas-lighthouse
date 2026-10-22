import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { flushSync } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  availableMonitors,
  currentMonitor,
  cursorPosition,
  getCurrentWindow,
  primaryMonitor,
  type Monitor,
} from "@tauri-apps/api/window";
import gsap from "gsap";
import { useGSAP } from "@gsap/react";
import { ArrowRight } from "lucide-react";
import { StatusBloub } from "./components/StatusBloub";
import { StatusDot } from "./components/StatusDot";
import { accessLabel, bytesPerSec, celsius, dash, pct, usedPct } from "./lib/format";
import { overlayBloubLive } from "./lib/overlayLive";
import { overlayDock, overlayIsMini, readOverlayDisplay } from "./lib/overlayPrefs";
import { parseBloubPin } from "./lib/bloubMood";
import { internetTitle, linkTitle } from "./lib/status";
import { summarizeAlerts } from "./lib/alerts";
import type { AppConfig, HealthEvent, ItemHealth, MetricsSnapshot, NavItem, OverlayDisplay, TempStatus } from "./lib/types";
import "./styles.css";
import "./overlay.css";

gsap.registerPlugin(useGSAP);

const OVERLAY_WIDTH = 520;
const CAPSULE_MINI = 73;
const OVERLAY_HEIGHT_BASE = 73;
const OVERLAY_LINE = 21;
const PILL_RADIUS = 999;
const LIQUID_EASE = "expo.out";
const MORPH_EASE = "power3.inOut";
const MORPH_DUR = 0.42;
const SNAP_PX = 56;
const EDGE_PAD = 10;
const HOVER_OPEN_MS = 200;
const LEAVE_CLOSE_MS = 140;
const PEEK_WATCH_MS = 320;

type ShortcutFit = "ports" | "names" | "wrap";

function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function tempOf(temps: TempStatus[], label: string): number | null {
  return temps.find((temp) => temp.label === label)?.celsius ?? null;
}

function overlayBox(mini: boolean, showAccess: boolean, rows: number): { width: number; height: number } {
  const extra = mini || !showAccess ? 0 : Math.max(0, rows - 1) * OVERLAY_LINE;
  return { width: OVERLAY_WIDTH, height: OVERLAY_HEIGHT_BASE + extra };
}

function chipRowCount(el: HTMLElement): number {
  const kids = Array.from(el.children) as HTMLElement[];
  if (kids.length === 0) return 1;
  return Math.max(1, new Set(kids.map((kid) => kid.offsetTop)).size);
}

function shellInnerSize(shell: HTMLElement): { width: number; height: number } {
  const cs = getComputedStyle(shell);
  const width = shell.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
  const height = shell.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
  return {
    width: Math.max(0, Math.floor(width)),
    height: Math.max(0, Math.floor(height)),
  };
}

function pinCircle(capsule: HTMLElement) {
  gsap.killTweensOf(capsule);
  gsap.set(capsule, {
    // Pinning may interrupt the entrance fade. Never retain its partial opacity.
    opacity: 1,
    x: 0,
    y: 0,
    scale: 1,
    width: CAPSULE_MINI,
    height: CAPSULE_MINI,
    maxWidth: CAPSULE_MINI,
    maxHeight: CAPSULE_MINI,
    minWidth: 0,
    minHeight: 0,
    flexGrow: 0,
    flexShrink: 0,
    flexBasis: CAPSULE_MINI,
    borderRadius: PILL_RADIUS,
  });
}

/** A tween killed by a later overwrite never resolves; don't let it strand `peekBusy`. */
function tweenSettled(tween: gsap.core.Tween, duration: number): Promise<void> {
  return Promise.race([
    Promise.resolve(tween).then(() => undefined),
    new Promise<void>((resolve) => {
      window.setTimeout(resolve, duration * 1000 + 150);
    }),
  ]);
}

function labeled(showLabels: boolean, label: string, value: string): string {
  return showLabels ? `${label} ${value}` : value;
}

function pointInMonitor(x: number, y: number, monitor: Monitor): boolean {
  const mx = monitor.position.x;
  const my = monitor.position.y;
  return x >= mx && y >= my && x < mx + monitor.size.width && y < my + monitor.size.height;
}

async function resolveOverlayMonitor(x: number, y: number): Promise<Monitor | null> {
  const monitors = await availableMonitors();
  const hit = monitors.find((monitor) => pointInMonitor(x, y, monitor));
  if (hit) return hit;
  return (await currentMonitor()) ?? (await primaryMonitor()) ?? monitors[0] ?? null;
}

type DockSide = "left" | "right" | null;

const OPAQUE_FILL_PNG =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR42mMQkxLHihiGlgQA7+IRwZjCBp0AAAAASUVORK5CYII=";

/** Opaque pixels inside the capsule; ancestor opacity must also finish at 1. */
function CapsuleOpaqueFill({ active, core = false }: { active: boolean; core?: boolean }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useLayoutEffect(() => {
    if (!active) return;
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d", { alpha: false });
    if (!ctx) return;
    canvas.width = 8;
    canvas.height = 64;
    const gradient = ctx.createLinearGradient(0, 0, 0, 64);
    gradient.addColorStop(0, "#2c352f");
    gradient.addColorStop(0.42, "#161a17");
    gradient.addColorStop(1, "#0c100e");
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, 8, 64);
  }, [active]);
  if (!active) return null;
  return (
    <>
      <img
        className={`liquid-fill${core ? " liquid-fill-core" : ""}`}
        alt=""
        draggable={false}
        src={OPAQUE_FILL_PNG}
      />
      <canvas
        ref={ref}
        className={`liquid-fill liquid-fill-shade${core ? " liquid-fill-core" : ""}`}
        aria-hidden
      />
    </>
  );
}

export function Overlay() {
  const rootRef = useRef<HTMLDivElement>(null);
  const capsuleRef = useRef<HTMLDivElement>(null);
  const hovered = useRef(false);
  const dragged = useRef(false);
  const allowSnap = useRef(false);
  const ignoreMove = useRef(false);
  const peekBusy = useRef(false);
  const dockRef = useRef<DockSide>(null);
  const peekedRef = useRef(false);
  const collapsedRef = useRef(false);
  const applyModeRef = useRef<(mini: boolean, side: DockSide) => Promise<void>>(async () => {});
  const hoverTimer = useRef(0);
  const leaveTimer = useRef(0);
  const ignoreMoveTimer = useRef(0);
  const ignoreMoveUntil = useRef(0);
  const flushHoverRef = useRef(() => {});
  const layoutRef = useRef({ showAccess: true, rows: 1 });
  const accessRowRef = useRef<HTMLDivElement>(null);
  const [snap, setSnap] = useState<MetricsSnapshot | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const [peeked, setPeeked] = useState(false);
  const [peeking, setPeeking] = useState(false);
  const [holdMini, setHoldMini] = useState(false);
  const [dockSide, setDockSide] = useState<DockSide>(null);
  const [ready, setReady] = useState(false);
  const [health, setHealth] = useState<Record<string, ItemHealth>>({});
  const [events, setEvents] = useState<HealthEvent[]>([]);
  const [fit, setFit] = useState<ShortcutFit>("ports");
  const [accessRows, setAccessRows] = useState(1);
  const [pointerInside, setPointerInside] = useState(false);
  const [windowVisible, setWindowVisible] = useState(true);

  const display: OverlayDisplay = readOverlayDisplay(config);
  const alerts = summarizeAlerts(events);
  const dock = dockSide;
  const shortcuts = useMemo(
    () => [...(config?.navItems ?? [])].sort((a, b) => a.order - b.order),
    [config?.navItems],
  );
  const shortcutKey = shortcuts.map((item) => item.id).join(",");
  layoutRef.current = { showAccess: display.showAccess, rows: accessRows };
  const visualMini = collapsed && !peeked;
  const hideCopy = visualMini || peeking;
  const shapeMini = visualMini || holdMini;
  // In dock mode the 47px backing stays directly under the equally sized bot.
  // Keeping it mounted avoids a transparent hand-off frame during the morph;
  // once expanded it remains fully covered by the bot and adds no visible ring.
  const opaqueCore = !display.opaque && collapsed;
  // DOM pointer-events cannot pass clicks through a native WebView window.
  // Clip only the resting mini state; keep the full surface during morphs.
  useLayoutEffect(() => {
    const capsule = capsuleRef.current;
    if (!ready || !capsule || peeking) return;
    if (!visualMini) {
      void invoke("set_overlay_clip", { rect: null }).catch(console.warn);
      return;
    }
    let frame = 0;
    let previous = "";
    const sync = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = capsule.getBoundingClientRect();
        const scale = window.devicePixelRatio;
        const rect = [
          Math.max(0, Math.floor(r.left * scale)),
          Math.max(0, Math.floor(r.top * scale)),
          Math.ceil(r.right * scale),
          Math.ceil(r.bottom * scale),
        ];
        const key = rect.join(",");
        if (key === previous || r.width <= 0 || r.height <= 0) return;
        previous = key;
        void invoke("set_overlay_clip", { rect }).catch((error) => {
          previous = "";
          console.warn(error);
        });
      });
    };
    const observer = new ResizeObserver(sync);
    observer.observe(capsule);
    window.addEventListener("resize", sync);
    sync();
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [ready, visualMini, peeking, dockSide]);
  const bloubLive = overlayBloubLive({
    shown: (config?.ui.overlay.visible ?? true) && windowVisible,
    collapsed,
    peeked,
    hovered: pointerInside,
  });
  const showPortLabels = display.showPorts && fit === "ports";
  const bothGreen = Boolean(
    snap &&
      display.showLink &&
      display.showInternet &&
      snap.link === "connected" &&
      snap.internet.state === "up",
  );

  function armIgnoreMove(ms = 450) {
    ignoreMove.current = true;
    const now = Date.now();
    // A shorter request must not shorten a longer guard, but it must never leave
    // the flag raised without a timer that lowers it again.
    const until = Math.max(now + ms, ignoreMoveUntil.current);
    ignoreMoveUntil.current = until;
    window.clearTimeout(ignoreMoveTimer.current);
    ignoreMoveTimer.current = window.setTimeout(() => {
      ignoreMove.current = false;
      flushHoverRef.current();
    }, Math.max(0, until - now));
  }

  const placeWindow = useCallback(async (mini: boolean, side: DockSide, keepY = false) => {
    const win = getCurrentWindow();
    const oldPos = await win.outerPosition();
    const oldSize = await win.outerSize();
    const monitor = await resolveOverlayMonitor(oldPos.x, oldPos.y);
    if (!monitor) return;
    const { showAccess, rows } = layoutRef.current;
    const box = overlayBox(mini, showAccess, rows);
    armIgnoreMove(450);
    const scale = monitor.scaleFactor;
    const width = Math.max(1, Math.round(box.width * scale));
    const height = Math.max(1, Math.round(box.height * scale));
    const mx = monitor.position.x;
    const my = monitor.position.y;
    const mw = monitor.size.width;
    const mh = monitor.size.height;
    const pad = Math.round(EDGE_PAD * scale);
    const minX = mx + pad;
    const maxX = mx + mw - width - pad;
    const minY = my + pad;
    const maxY = my + mh - height - pad;
    let x = oldPos.x;
    if (side === "left") x = minX;
    else if (side === "right") x = Math.max(minX, maxX);
    else x = Math.min(Math.max(oldPos.x, minX), Math.max(minX, maxX));
    let y = oldPos.y;
    if (side && !keepY) {
      y = oldPos.y + Math.round((oldSize.height - height) / 2);
    }
    y = Math.min(Math.max(y, minY), Math.max(minY, maxY));
    if (oldPos.x === x && oldPos.y === y && oldSize.width === width && oldSize.height === height) {
      return;
    }
    await invoke("place_overlay", { x, y, width, height });
  }, []);

  const morphCapsule = useCallback((mini: boolean, instant = false) => {
    const capsule = capsuleRef.current;
    const shell = rootRef.current;
    if (!capsule || !shell) return Promise.resolve();
    const duration = instant || prefersReducedMotion() ? 0 : MORPH_DUR;
    // A shape tween takes over from the entrance tween (overwrite: true).
    // Finish its fade explicitly, including values above the old 0.2 threshold.
    gsap.killTweensOf(capsule, "opacity");
    gsap.set(capsule, { opacity: 1 });
    const inner = shellInnerSize(shell);
    const width = mini ? CAPSULE_MINI : Math.max(CAPSULE_MINI, inner.width);
    const height = mini ? CAPSULE_MINI : Math.max(CAPSULE_MINI, inner.height);
    const vars = {
      width,
      height,
      maxWidth: width,
      maxHeight: height,
      minWidth: 0,
      minHeight: 0,
      flexGrow: 0,
      flexShrink: 0,
      flexBasis: width,
      borderRadius: PILL_RADIUS,
      x: 0,
      y: 0,
      scale: 1,
    };
    if (duration === 0) {
      gsap.set(capsule, vars);
      return Promise.resolve();
    }
    if (!mini) {
      return tweenSettled(
        gsap.fromTo(
          capsule,
          {
            width: CAPSULE_MINI,
            height: CAPSULE_MINI,
            maxWidth: CAPSULE_MINI,
            maxHeight: CAPSULE_MINI,
            flexBasis: CAPSULE_MINI,
            x: 0,
            y: 0,
            scale: 1,
          },
          { ...vars, duration, ease: MORPH_EASE, overwrite: true },
        ),
        duration,
      );
    }
    return tweenSettled(
      gsap.to(capsule, { ...vars, duration, ease: MORPH_EASE, overwrite: true }),
      duration,
    );
  }, []);

  const expandPeek = useCallback(async () => {
    if (!collapsedRef.current || peekedRef.current || peekBusy.current || dragged.current) return;
    peekBusy.current = true;
    armIgnoreMove(800);
    window.clearTimeout(leaveTimer.current);
    const capsule = capsuleRef.current;
    try {
      flushSync(() => {
        setPeeking(true);
        setHoldMini(true);
      });
      await invoke("set_overlay_clip", { rect: null });
      if (capsule) pinCircle(capsule);
      peekedRef.current = true;
      flushSync(() => {
        setPeeked(true);
      });
      if (capsule) pinCircle(capsule);
      await morphCapsule(false);
    } finally {
      peekBusy.current = false;
      setHoldMini(false);
      setPeeking(false);
      armIgnoreMove(400);
    }
  }, [morphCapsule]);

  const collapsePeek = useCallback(async () => {
    if (!collapsedRef.current || !peekedRef.current || peekBusy.current) return;
    peekBusy.current = true;
    armIgnoreMove(800);
    window.clearTimeout(hoverTimer.current);
    const capsule = capsuleRef.current;
    try {
      flushSync(() => {
        setPeeking(true);
      });
      await morphCapsule(true);
      peekedRef.current = false;
      flushSync(() => {
        setPeeked(false);
        setHoldMini(false);
      });
      if (capsule) pinCircle(capsule);
    } finally {
      peekBusy.current = false;
      setPeeking(false);
      setHoldMini(false);
      armIgnoreMove(400);
    }
  }, [morphCapsule]);

  function flushHoverMode() {
    if (peekBusy.current || dragged.current || ignoreMove.current) return;
    window.clearTimeout(hoverTimer.current);
    window.clearTimeout(leaveTimer.current);
    if (!collapsedRef.current) return;
    if (hovered.current && !peekedRef.current) {
      hoverTimer.current = window.setTimeout(() => {
        void expandPeek();
      }, HOVER_OPEN_MS);
      return;
    }
    if (!hovered.current && peekedRef.current) {
      leaveTimer.current = window.setTimeout(() => {
        void collapsePeek();
      }, LEAVE_CLOSE_MS);
    }
  }
  flushHoverRef.current = flushHoverMode;

  // A `pointerleave` that lands while a morph or a drag guard is active gets dropped, and
  // the WebView never resends it — the peek then stays open for good. While peeked, check
  // the real cursor against the window box instead of trusting that single event.
  useEffect(() => {
    if (!collapsed || !peeked) return;
    const win = getCurrentWindow();
    let alive = true;
    const check = async () => {
      if (!alive || peekBusy.current) return;
      if (ignoreMove.current && Date.now() >= ignoreMoveUntil.current) ignoreMove.current = false;
      const [cursor, pos, size] = await Promise.all([
        cursorPosition(),
        win.outerPosition(),
        win.outerSize(),
      ]);
      const inside =
        cursor.x >= pos.x &&
        cursor.y >= pos.y &&
        cursor.x < pos.x + size.width &&
        cursor.y < pos.y + size.height;
      if (!alive || inside || peekBusy.current || !peekedRef.current) return;
      hovered.current = false;
      setPointerInside(false);
      window.clearTimeout(hoverTimer.current);
      capsuleRef.current?.classList.remove("is-hover");
      void collapsePeek();
    };
    const timer = window.setInterval(() => {
      void check().catch(() => undefined);
    }, PEEK_WATCH_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [collapsePeek, collapsed, peeked]);

  useEffect(() => {
    document.documentElement.classList.add("overlay-root");
    document.body.classList.add("overlay-root");
    const win = getCurrentWindow();
    void win.isVisible().then(setWindowVisible).catch(() => undefined);
    invoke<MetricsSnapshot>("get_snapshot").then(setSnap).catch(() => setSnap(null));
    invoke<Record<string, ItemHealth>>("get_health").then(setHealth).catch(() => undefined);
    invoke<HealthEvent[]>("get_events").then(setEvents).catch(() => undefined);
    invoke<AppConfig>("get_config")
      .then((cfg) => {
        setConfig(cfg);
        setWindowVisible(cfg.ui.overlay.visible);
        const mini = overlayIsMini(cfg);
        collapsedRef.current = mini;
        setCollapsed(mini);
        dockRef.current = overlayDock(cfg);
        setDockSide(dockRef.current);
      })
      .catch(() => undefined);
    const unlistenMetrics = listen<MetricsSnapshot>("metrics", (event) => setSnap(event.payload));
    const unlistenHealth = listen<Record<string, ItemHealth>>("health", (event) => setHealth(event.payload));
    const unlistenEvents = listen<HealthEvent[]>("events", (event) => setEvents(event.payload));
    const unlistenConfig = listen<AppConfig>("config", (event) => {
      setConfig(event.payload);
      setWindowVisible(event.payload.ui.overlay.visible);
      const mini = overlayIsMini(event.payload);
      if (mini !== collapsedRef.current) {
        void applyModeRef.current(mini, mini ? overlayDock(event.payload) : null);
        return;
      }
      dockRef.current = overlayDock(event.payload);
      setDockSide(dockRef.current);
    });
    return () => {
      window.clearTimeout(hoverTimer.current);
      window.clearTimeout(leaveTimer.current);
      window.clearTimeout(ignoreMoveTimer.current);
      void unlistenMetrics.then((fn) => fn());
      void unlistenHealth.then((fn) => fn());
      void unlistenEvents.then((fn) => fn());
      void unlistenConfig.then((fn) => fn());
      document.documentElement.classList.remove("overlay-root");
      document.body.classList.remove("overlay-root");
    };
  }, []);

  function openMain() {
    void invoke("show_main_window");
  }

  const menuStamp = useRef(0);

  async function openPop(kind: "menu" | "search", event?: { clientX: number; clientY: number }) {
    const now = Date.now();
    if (kind === "menu" && now - menuStamp.current < 400) return;
    if (kind === "menu") menuStamp.current = now;
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    const factor = await win.scaleFactor();
    const x = Math.round(pos.x + (event?.clientX ?? 0) * factor);
    const y = Math.round(pos.y + (event?.clientY ?? 0) * factor);
    await invoke("open_overlay_pop", { kind, x, y });
  }

  function openShortcut(event: MouseEvent, item: NavItem) {
    event.preventDefault();
    event.stopPropagation();
    void invoke("open_access", { port: item.port });
  }

  const applyMode = useCallback(
    async (mini: boolean, side: DockSide) => {
      if (!mini) await invoke("set_overlay_clip", { rect: null });
      peekBusy.current = false;
      const win = getCurrentWindow();
      armIgnoreMove(500);
      peekedRef.current = false;
      setPeeked(false);
      setHoldMini(false);
      collapsedRef.current = mini;
      setCollapsed(mini);
      dockRef.current = side;
      setDockSide(side);
      await placeWindow(mini, side);
      if (mini && capsuleRef.current) pinCircle(capsuleRef.current);
      const pos = await win.outerPosition();
      await invoke("set_overlay_state", {
        patch: { x: pos.x, y: pos.y, collapsed: mini, dock: side ?? "none" },
      });
      requestAnimationFrame(() => {
        void morphCapsule(mini, true);
      });
      armIgnoreMove(400);
    },
    [morphCapsule, placeWindow],
  );
  applyModeRef.current = applyMode;

  const maybeSnap = useCallback(async () => {
    if (peekBusy.current) return;
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    const size = await win.outerSize();
    const monitor = await resolveOverlayMonitor(pos.x + size.width / 2, pos.y + size.height / 2);
    if (!monitor) return;
    if (!display.snapHide) {
      await applyMode(false, null);
      return;
    }
    const mx = monitor.position.x;
    const mw = monitor.size.width;
    const threshold = Math.round(SNAP_PX * monitor.scaleFactor);
    const leftGap = pos.x - mx;
    const rightGap = mx + mw - (pos.x + size.width);
    let side: DockSide = null;
    if (leftGap <= threshold) side = "left";
    else if (rightGap <= threshold) side = "right";
    await applyMode(side != null, side);
  }, [applyMode, display.snapHide]);

  const parkedRef = useRef(false);
  useEffect(() => {
    if (!config || parkedRef.current) return;
    parkedRef.current = true;
    const side = overlayDock(config);
    if (side && overlayIsMini(config)) void applyMode(true, side);
  }, [applyMode, config]);

  useEffect(() => {
    const win = getCurrentWindow();
    let timer: number | undefined;
    const unlisten = win.onMoved(() => {
      if (ignoreMove.current || !allowSnap.current || peekBusy.current) return;
      dragged.current = true;
      window.clearTimeout(hoverTimer.current);
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        allowSnap.current = false;
        void maybeSnap().finally(() => {
          dragged.current = false;
          flushHoverRef.current();
        });
      }, 280);
    });
    return () => {
      window.clearTimeout(timer);
      void unlisten.then((fn) => fn());
    };
  }, [maybeSnap]);

  useEffect(() => {
    if (!display.snapHide && collapsedRef.current) void applyMode(false, null);
  }, [applyMode, display.snapHide]);

  useEffect(() => {
    setFit(display.showPorts ? "ports" : "names");
    setAccessRows(1);
  }, [display.showPorts, shortcutKey]);

  useLayoutEffect(() => {
    const el = accessRowRef.current;
    if (!ready || visualMini || peeking || hideCopy || !display.showAccess || !el) return;
    const overflowing = el.scrollWidth > el.clientWidth + 2;
    if (overflowing && fit === "ports") {
      setFit("names");
      return;
    }
    if (overflowing && fit === "names") {
      setFit("wrap");
      return;
    }
    const rows = fit === "wrap" ? chipRowCount(el) : 1;
    if (rows !== accessRows) setAccessRows(rows);
  }, [accessRows, display.showAccess, fit, hideCopy, peeking, ready, shortcutKey, visualMini, showPortLabels]);

  const accessCountRef = useRef(0);
  const showAccessRef = useRef(true);
  const accessRowsRef = useRef(1);
  useEffect(() => {
    const count = shortcuts.length;
    if (!ready) {
      accessCountRef.current = count;
      showAccessRef.current = display.showAccess;
      accessRowsRef.current = accessRows;
      return;
    }
    if (
      count === accessCountRef.current &&
      display.showAccess === showAccessRef.current &&
      accessRows === accessRowsRef.current
    ) {
      return;
    }
    accessCountRef.current = count;
    showAccessRef.current = display.showAccess;
    accessRowsRef.current = accessRows;
    if (peekBusy.current) return;
    if (collapsedRef.current && !peekedRef.current) return;
    void placeWindow(false, dockRef.current, true);
    requestAnimationFrame(() => {
      void morphCapsule(false);
    });
  }, [accessRows, display.showAccess, morphCapsule, placeWindow, ready, shortcuts.length]);

  useGSAP(
    () => {
      const capsule = capsuleRef.current;
      if (!capsule) return;
      setReady(true);
      if (prefersReducedMotion()) {
        gsap.set(capsule, { y: 0, scale: 1, opacity: 1 });
        if (collapsedRef.current) void morphCapsule(true, true);
        return;
      }
      gsap.fromTo(
        capsule,
        { y: collapsedRef.current ? 0 : 36, scale: 0.972, opacity: 0 },
        {
          y: 0,
          scale: 1,
          opacity: 1,
          duration: 0.46,
          ease: LIQUID_EASE,
          onComplete: () => {
            if (collapsedRef.current && !peekedRef.current) void morphCapsule(true, true);
          },
        },
      );
    },
    { scope: rootRef },
  );

  function liftTarget() {
    return hovered.current ? { y: -1, scale: 1 } : { y: 0, scale: 1 };
  }

  function hoverIn() {
    hovered.current = true;
    setPointerInside(true);
    if (!dragged.current) allowSnap.current = false;
    window.clearTimeout(leaveTimer.current);
    capsuleRef.current?.classList.add("is-hover");
    if (collapsedRef.current) {
      flushHoverMode();
      return;
    }
    const el = capsuleRef.current;
    if (!el || prefersReducedMotion()) return;
    gsap.to(el, { ...liftTarget(), duration: 0.2, ease: "power2.out", overwrite: "auto" });
  }

  function hoverOut(event: { relatedTarget: EventTarget | null }) {
    const next = event.relatedTarget;
    if (next instanceof Node && capsuleRef.current?.contains(next)) return;
    hovered.current = false;
    setPointerInside(false);
    window.clearTimeout(hoverTimer.current);
    capsuleRef.current?.classList.remove("is-hover");
    if (peekBusy.current || ignoreMove.current || dragged.current) return;
    if (collapsedRef.current) {
      flushHoverMode();
      return;
    }
    const el = capsuleRef.current;
    if (!el) return;
    gsap.to(el, { y: 0, scale: 1, duration: 0.26, ease: "power2.out", overwrite: "auto" });
  }

  function pressIn() {
    if (collapsedRef.current) return;
    const el = capsuleRef.current;
    if (!el || prefersReducedMotion()) return;
    gsap.to(el, { y: 0, scale: 0.97, duration: 0.1, ease: "power2.out", overwrite: "auto" });
  }

  function pressOut() {
    if (collapsedRef.current) return;
    const el = capsuleRef.current;
    if (!el || prefersReducedMotion()) return;
    gsap.to(el, { ...liftTarget(), duration: 0.24, ease: "power2.out", overwrite: "auto" });
  }

  const digits = display.precision;
  const memUsed =
    snap && snap.mem.totalKb != null && snap.mem.availableKb != null
      ? snap.mem.totalKb - snap.mem.availableKb
      : null;
  const memPct = snap && memUsed != null && snap.mem.totalKb ? usedPct(memUsed, snap.mem.totalKb) : null;
  const cpuTemp = snap ? tempOf(snap.temps, "CPU") : null;
  const memTemp = snap ? tempOf(snap.temps, "内存") : null;

  const metrics: string[] = [];
  if (snap) {
    if (display.showCpu) {
      metrics.push(labeled(display.showLabels, "CPU", dash(snap.cpu.usagePct, (n) => pct(n, digits))));
    }
    if (display.showCpuTemp) {
      metrics.push(labeled(display.showLabels, "温度", dash(cpuTemp, (n) => celsius(n, digits))));
    }
    if (display.showMem) {
      metrics.push(labeled(display.showLabels, "内存", dash(memPct, (n) => pct(n, digits))));
    }
    if (display.showMemTemp) {
      metrics.push(labeled(display.showLabels, "内存温度", dash(memTemp, (n) => celsius(n, digits))));
    }
    if (display.showNet && !visualMini) {
      metrics.push(`↓ ${dash(snap.net.rxBytesPerSec, bytesPerSec)}`);
      metrics.push(`↑ ${dash(snap.net.txBytesPerSec, bytesPerSec)}`);
    }
  }

  const shellClass = [
    "overlay-shell",
    shapeMini ? "is-mini" : "",
    collapsed ? "is-docked" : "",
    dock === "left" ? "is-dock-left" : "",
    dock === "right" ? "is-dock-right" : "",
    peeked ? "is-peek" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      ref={rootRef}
      className={shellClass}
      {...(shapeMini ? {} : { "data-tauri-drag-region": true })}
      onMouseDown={() => {
        allowSnap.current = true;
      }}
    >
      <div
        ref={capsuleRef}
        className={`liquid-capsule${shapeMini ? " is-mini" : ""}${ready ? " is-visible" : ""}${display.opaque ? " is-opaque" : ""}`}
        data-tauri-drag-region
        onPointerEnter={hoverIn}
        onPointerLeave={hoverOut}
        onMouseDown={pressIn}
        onMouseUp={(event) => {
          pressOut();
          if (event.button === 2) void openPop("menu", event);
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void openPop("menu", event);
        }}
      >
        <CapsuleOpaqueFill active={display.opaque} />
        <CapsuleOpaqueFill active={opaqueCore} core />
        <span className="liquid-frost" />
        <span className="liquid-glass-bottom" />
        <span className="liquid-shine" />
        <span className="liquid-chroma liquid-chroma-left" />
        <span className="liquid-chroma liquid-chroma-right" />
        <span className="liquid-spec" />
        <div className="overlay-body" data-tauri-drag-region>
          <div className={`overlay-copy${hideCopy ? " is-hidden" : ""}`} data-tauri-drag-region>
            <div className="overlay-row overlay-main" data-tauri-drag-region>
              <div className="overlay-metrics" data-tauri-drag-region>
                {snap ? (
                  <>
                    {display.showLink || display.showInternet ? (
                      <span
                        className={`overlay-dots${bothGreen ? " is-synced" : ""}`}
                        key={bothGreen ? "dots-sync" : "dots-free"}
                      >
                        {display.showLink ? (
                          <StatusDot kind="link" state={snap.link} title={linkTitle(snap)} compact />
                        ) : null}
                        {display.showInternet ? (
                          <StatusDot kind="internet" state={snap.internet.state} title={internetTitle(snap)} compact />
                        ) : null}
                      </span>
                    ) : null}
                    {metrics.map((text, index) => (
                      <span key={`m-${index}`}>{text}</span>
                    ))}
                  </>
                ) : (
                  <span className="muted">正在连接…</span>
                )}
              </div>
            </div>
            {display.showAccess ? (
              <div className="overlay-access-slot">
                <div
                  ref={accessRowRef}
                  className={`overlay-row overlay-access${fit === "wrap" ? " is-wrap" : ""}`}
                >
                  {shortcuts.length === 0 ? (
                    <span className="muted overlay-chip">到导航页收录后可在此打开</span>
                  ) : (
                    shortcuts.map((item) => (
                      <button
                        type="button"
                        className="overlay-chip no-drag"
                        key={item.id}
                        title={`打开 ${accessLabel(item, true)}`}
                        onMouseDown={(event) => event.stopPropagation()}
                        onClick={(event) => openShortcut(event, item)}
                      >
                        <span className="overlay-chip-icon" aria-hidden="true">
                          <ArrowRight size={16} strokeWidth={2.4} />
                        </span>
                        <span className="overlay-chip-label">{accessLabel(item, showPortLabels)}</span>
                      </button>
                    ))
                  )}
                </div>
              </div>
            ) : null}
          </div>
          <StatusBloub
            variant="overlay"
            snap={snap}
            health={health}
            alerts={alerts}
            pin={parseBloubPin(config?.ui.bloubPin)}
            live={bloubLive}
            onActivate={openMain}
          />
        </div>
      </div>
    </div>
  );
}
