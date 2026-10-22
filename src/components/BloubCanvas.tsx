import { useEffect, useId, useRef, useState } from "react";
import { lookAtPointer, pointerFromScreen, type ScreenPointer } from "../lib/botLook";
import { bloubLoopActive } from "../lib/overlayLive";
import { NOTIF_BLUE, type DotRender } from "../vendor/bloub/decor";
import { BotEngine, type BotFrame } from "../vendor/bloub/engine";
import { DEMI_VIEWBOX, RAYON } from "../vendor/bloub/repere";
import { mixHex } from "../vendor/bloub/skins";
import { POSES, type StateId } from "../vendor/bloub/states";

function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function BloubCanvas({
  state,
  size = 56,
  ink,
  paper,
  followPointer = false,
  live = true,
}: {
  state: StateId;
  size?: number;
  ink: string;
  paper: string;
  followPointer?: boolean;
  live?: boolean;
}) {
  const uid = useId().replace(/:/g, "");
  const svgRef = useRef<SVGSVGElement>(null);
  const engineRef = useRef<BotEngine | null>(null);
  if (!engineRef.current) {
    engineRef.current = new BotEngine(RAYON, state);
  }
  const clockRef = useRef(0);
  const stateRef = useRef(state);
  const followRef = useRef(false);
  const aimingRef = useRef(false);
  const pointerRef = useRef<ScreenPointer | null>(null);
  const [frame, setFrame] = useState<BotFrame>(() => engineRef.current!.sample(0));
  const reducedMotion = prefersReducedMotion();
  const looping = bloubLoopActive(live, reducedMotion);
  const tracking = looping && followPointer && state === "idle";
  followRef.current = tracking;
  useEffect(() => {
    if (!tracking) {
      pointerRef.current = null;
      return;
    }
    let cancelled = false;
    let timer = 0;
    void import("@tauri-apps/api/window")
      .then(({ cursorPosition, currentMonitor, getCurrentWindow }) => {
        if (cancelled) return;
        const win = getCurrentWindow();
        const poll = async () => {
          try {
            const [cursor, origin, scale, monitor] = await Promise.all([
              cursorPosition(),
              win.innerPosition(),
              win.scaleFactor(),
              currentMonitor(),
            ]);
            if (cancelled) return;
            const span = monitor?.size;
            if (!span || span.width < 1 || span.height < 1) return;
            pointerRef.current = {
              cursorX: cursor.x,
              cursorY: cursor.y,
              originX: origin.x,
              originY: origin.y,
              scale,
              spanX: span.width,
              spanY: span.height,
            };
          } catch {
            pointerRef.current = null;
          }
        };
        void poll();
        timer = window.setInterval(() => void poll(), 32);
      })
      .catch(() => {
        pointerRef.current = null;
      });
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      pointerRef.current = null;
    };
  }, [tracking]);

  useEffect(() => {
    const engine = engineRef.current!;
    const now = clockRef.current;
    const stateChanged = stateRef.current !== state;
    if (stateChanged) {
      engine.setState(state, now);
      stateRef.current = state;
    }
    if (!looping) {
      aimingRef.current = false;
      if (reducedMotion || stateChanged) {
        engine.setLook(null, now);
        setFrame(engine.sample(now + (POSES[state] ?? 1)));
      }
      return;
    }
    let raf = 0;
    let last = performance.now();
    const tick = (stamp: number) => {
      const dt = Math.min(0.05, (stamp - last) / 1000);
      last = stamp;
      clockRef.current += dt;
      const clock = clockRef.current;
      const pointer = followRef.current ? pointerRef.current : null;
      const box = svgRef.current?.getBoundingClientRect();
      const aim = pointer && box ? pointerFromScreen(pointer, box) : null;
      if (aim) {
        engine.setLook(lookAtPointer(aim.nx, aim.ny), clock);
        aimingRef.current = true;
      } else if (aimingRef.current) {
        engine.setLook(null, clock);
        aimingRef.current = false;
      }
      setFrame(engine.sample(clock));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [looping, reducedMotion, state]);
  const vb = DEMI_VIEWBOX;
  const maskId = `bloub-mask-${uid}`;

  function dotFill(dot: DotRender): string {
    if (dot.color) return dot.color;
    if (dot.depth === undefined) return ink;
    return mixHex(paper, ink, dot.depth);
  }

  return (
    <svg
      ref={svgRef}
      width={size}
      height={size}
      viewBox={`${-vb} ${-vb} ${vb * 2} ${vb * 2}`}
      role="img"
      aria-hidden="true"
      className="bloub-svg"
    >
      <defs>
        <mask id={maskId} maskUnits="userSpaceOnUse" x={-vb} y={-vb} width={vb * 2} height={vb * 2}>
          <path d={frame.bodyPath} fill="#fff" />
          {frame.eyes.map((eye, index) => (
            <path key={index} d={eye.d} transform={eye.matrix} opacity={eye.alpha} fill="#000" />
          ))}
          {frame.notch ? <circle cx={frame.notch.x} cy={frame.notch.y} r={frame.notch.r} fill="#000" /> : null}
        </mask>
        {frame.arcs.map((arc) => (
          <linearGradient
            key={arc.id}
            id={`${uid}-${arc.id}`}
            gradientUnits="userSpaceOnUse"
            x1={arc.grad.x1}
            y1={arc.grad.y1}
            x2={arc.grad.x2}
            y2={arc.grad.y2}
          >
            {arc.grad.stops.map((color, index) => (
              <stop
                key={index}
                offset={arc.grad.stops.length === 1 ? 0 : index / (arc.grad.stops.length - 1)}
                stopColor={color}
              />
            ))}
          </linearGradient>
        ))}
      </defs>
      <g fill="none" strokeLinecap="round">
        {frame.arcs.map((arc) => (
          <path
            key={`b${arc.id}`}
            d={arc.back}
            stroke={`url(#${uid}-${arc.id})`}
            strokeWidth={arc.width}
            opacity={arc.opacity}
          />
        ))}
      </g>
      {frame.dotsBehind
        ? frame.dots.map((dot, index) =>
            dot.d ? (
              <path
                key={`pb${index}`}
                d={dot.d}
                fill={dotFill(dot)}
                opacity={dot.opacity}
                transform={`translate(${dot.x} ${dot.y}) rotate(${dot.rot ?? 0}) scale(${RAYON})`}
              />
            ) : (
              <circle key={`pb${index}`} cx={dot.x} cy={dot.y} r={dot.r} fill={dotFill(dot)} opacity={dot.opacity} />
            ),
          )
        : null}
      <g opacity={frame.bodyAlpha}>
        <path d={frame.bodyPath} fill={paper} />
        <g mask={`url(#${maskId})`}>
          <rect x={-vb} y={-vb} width={vb * 2} height={vb * 2} fill={ink} />
        </g>
      </g>
      {!frame.dotsBehind
        ? frame.dots.map((dot, index) =>
            dot.d ? (
              <path
                key={`pf${index}`}
                d={dot.d}
                fill={dotFill(dot)}
                opacity={dot.opacity}
                transform={`translate(${dot.x} ${dot.y}) rotate(${dot.rot ?? 0}) scale(${RAYON})`}
              />
            ) : (
              <circle key={`pf${index}`} cx={dot.x} cy={dot.y} r={dot.r} fill={dotFill(dot)} opacity={dot.opacity} />
            ),
          )
        : null}
      {frame.notif ? <circle cx={frame.notif.x} cy={frame.notif.y} r={frame.notif.r} fill={NOTIF_BLUE} /> : null}
      <g fill="none" strokeLinecap="round">
        {frame.arcs.map((arc) => (
          <path
            key={`f${arc.id}`}
            d={arc.front}
            stroke={`url(#${uid}-${arc.id})`}
            strokeWidth={arc.width}
            opacity={arc.opacity}
          />
        ))}
      </g>
    </svg>
  );
}
