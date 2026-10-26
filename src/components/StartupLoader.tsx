import { useEffect, useRef, useState } from "react";
import { useGSAP } from "@gsap/react";
import gsap from "gsap";

gsap.registerPlugin(useGSAP);

type StartupLoaderProps = {
  ready: boolean;
  onDismiss: () => void;
};

export function StartupLoader({ ready, onDismiss }: StartupLoaderProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [introComplete, setIntroComplete] = useState(false);
  const [waitExpired, setWaitExpired] = useState(false);

  useEffect(() => {
    const timeout = window.setTimeout(() => setWaitExpired(true), 5000);
    return () => window.clearTimeout(timeout);
  }, []);

  useGSAP(
    () => {
      const root = rootRef.current;
      if (!root) return;

      const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      const tile = root.querySelector("[data-splash='tile']");
      const tower = root.querySelector("[data-splash='tower']");
      const towerWindow = root.querySelector("[data-splash='tower-window']");
      const house = root.querySelector("[data-splash='house']");
      const lamp = root.querySelector("[data-splash='lamp']");
      const signal = root.querySelector("[data-splash='signal']");
      const glow = root.querySelector("[data-splash='glow']");
      const copy = root.querySelector("[data-splash='copy']");
      const status = root.querySelector("[data-splash='status']");

      if (reduceMotion) {
        gsap.set([tile, tower, towerWindow, house, lamp, signal, copy, status], {
          autoAlpha: 1,
          clearProps: "transform,filter",
        });
        gsap.set(glow, { autoAlpha: 0.42, scale: 1 });
        setIntroComplete(true);
        return;
      }

      const timeline = gsap.timeline({
        defaults: { ease: "power2.out" },
        onComplete: () => setIntroComplete(true),
      });

      timeline
        .addLabel("anticipation", 0)
        .fromTo(
          tile,
          { autoAlpha: 0, scale: 0.84, transformOrigin: "50% 50%" },
          { autoAlpha: 1, scale: 1.025, duration: 0.86, ease: "expo.out" },
          "anticipation",
        )
        .to(tile, { scale: 1, duration: 0.34, ease: "power2.out" }, 0.86)
        .fromTo(
          glow,
          { autoAlpha: 0, scale: 0.56, transformOrigin: "50% 50%" },
          { autoAlpha: 0.42, scale: 1, duration: 0.92, ease: "power3.out" },
          0.08,
        )
        .addLabel("lighthouse", 0.28)
        .fromTo(
          house,
          { autoAlpha: 0, y: 9, scale: 0.86, transformOrigin: "50% 100%" },
          { autoAlpha: 1, y: -1, scale: 1.035, duration: 0.54, ease: "expo.out" },
          "lighthouse",
        )
        .to(house, { y: 0, scale: 1, duration: 0.32, ease: "power2.out" }, 0.82)
        .fromTo(
          tower,
          { autoAlpha: 0, y: 8, scaleY: 0.72, transformOrigin: "50% 100%" },
          { autoAlpha: 1, y: -0.8, scaleY: 1.025, duration: 0.58, ease: "expo.out" },
          0.38,
        )
        .to(tower, { y: 0, scaleY: 1, duration: 0.34, ease: "power2.out" }, 0.94)
        .addLabel("windows", 0.62)
        .fromTo(
          towerWindow,
          { autoAlpha: 0, scale: 0.72, transformOrigin: "50% 50%" },
          { autoAlpha: 1, scale: 1.08, duration: 0.24, ease: "back.out(1.35)" },
          "windows",
        )
        .to(towerWindow, { scale: 1, duration: 0.22, ease: "power2.out" }, 0.86)
        .fromTo(
          lamp,
          { autoAlpha: 0, scale: 0.76, transformOrigin: "50% 50%" },
          { autoAlpha: 1, scale: 1.1, duration: 0.24, ease: "back.out(1.45)" },
          0.7,
        )
        .to(lamp, { scale: 1, duration: 0.22, ease: "power2.out" }, 0.94)
        .addLabel("returning-light", 0.58)
        .fromTo(
          signal,
          {
            autoAlpha: 0,
            x: 18,
            y: 3,
            rotation: -10,
            scale: 0.7,
            transformOrigin: "0% 50%",
            filter: "drop-shadow(0 0 0 rgba(109, 93, 252, 0))",
          },
          {
            autoAlpha: 1,
            x: -1.7,
            y: -0.8,
            rotation: 2,
            scale: 1.045,
            filter: "drop-shadow(0 0 3px rgba(109, 93, 252, 0.36))",
            duration: 0.48,
            ease: "expo.out",
          },
          "returning-light",
        )
        .to(
          signal,
          {
            x: 0,
            y: 0,
            rotation: 0,
            scale: 1,
            filter: "drop-shadow(0 0 0 rgba(109, 93, 252, 0))",
            duration: 0.36,
            ease: "power2.out",
          },
          1.04,
        )
        .fromTo(
          copy,
          { autoAlpha: 0, y: 12 },
          { autoAlpha: 1, y: 0, duration: 0.38, ease: "power3.out" },
          0.76,
        )
        .fromTo(
          status,
          { autoAlpha: 0, y: 8 },
          { autoAlpha: 1, y: 0, duration: 0.34, ease: "power2.out" },
          0.92,
        )
        .to({}, { duration: 0.08 }, 1.42);
    },
    { scope: rootRef },
  );

  useGSAP(
    () => {
      if ((!ready && !waitExpired) || !introComplete || !rootRef.current) return;

      const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (reduceMotion) {
        onDismiss();
        return;
      }

      const timeline = gsap.timeline({ onComplete: onDismiss });
      timeline
        .to("[data-splash='copy'], [data-splash='status']", {
          autoAlpha: 0,
          y: -8,
          duration: 0.18,
          ease: "power2.in",
        })
        .to(
          ".startup-logo-stage",
          { scale: 1.035, duration: 0.26, ease: "power2.inOut" },
          0,
        )
        .to(rootRef.current, { autoAlpha: 0, duration: 0.3, ease: "power2.inOut" }, 0.12);
    },
    { dependencies: [ready, waitExpired, introComplete], scope: rootRef, revertOnUpdate: true },
  );

  return (
    <div ref={rootRef} className="startup-loader" role="status" aria-live="polite" aria-label="拾光灯塔正在启动">
      <div className="startup-loader-noise" aria-hidden="true" />
      <div className="startup-logo-stage">
        <div className="startup-glow" data-splash="glow" aria-hidden="true" />
        <svg className="startup-logo" viewBox="0 0 80 80" role="img" aria-labelledby="startup-logo-title">
          <title id="startup-logo-title">拾光灯塔</title>
          <g data-splash="tile" style={{ transformBox: "fill-box", transformOrigin: "center" }}>
            <rect x="4" y="4" width="72" height="72" rx="16" fill="#fff" />
          </g>
          <g transform="translate(7.2 7.2) scale(.82)">
            <path
              data-splash="tower"
              fill="#6d5dfc"
              d="M28 38h24l4 34H24l4-34Z"
              style={{ transformBox: "fill-box", transformOrigin: "center bottom" }}
            />
            <rect
              data-splash="tower-window"
              x="35"
              y="48"
              width="10"
              height="12"
              fill="#fff"
              style={{ transformBox: "fill-box", transformOrigin: "center" }}
            />
            <path
              data-splash="house"
              fill="#6d5dfc"
              d="M16 26h10V11L40 2l14 9v27H16V26Z"
              style={{ transformBox: "fill-box", transformOrigin: "center bottom" }}
            />
            <path
              data-splash="lamp"
              fill="#fff"
              d="M34 26V16l6-5 6 5v10H34Z"
              style={{ transformBox: "fill-box", transformOrigin: "center" }}
            />
            <path
              data-splash="signal"
              fill="#6d5dfc"
              d="m73 16-16 13 16 13 6-6-10-7 10-7-6-6Z"
              style={{ transformBox: "fill-box", transformOrigin: "left center" }}
            />
          </g>
        </svg>
      </div>
      <div className="startup-copy" data-splash="copy">
        <strong>拾光灯塔</strong>
        <span>本机与 NAS 服务导航</span>
      </div>
      <p className={`startup-status${ready ? " is-ready" : ""}`} data-splash="status">
        <span aria-hidden="true" />
        {ready ? "灯塔已点亮" : waitExpired ? "正在打开应用…" : "正在连接本机与 NAS…"}
      </p>
    </div>
  );
}
