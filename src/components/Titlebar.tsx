import { useEffect, useState, type MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";

export function Titlebar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let cancelled = false;
    try {
      const current = getCurrentWindow();
      const sync = () => {
        void current.isMaximized().then((value) => {
          if (!cancelled) setMaximized(value);
        });
      };
      sync();
      const unlisten = current.onResized(() => sync());
      return () => {
        cancelled = true;
        void unlisten.then((fn) => fn());
      };
    } catch {
      return undefined;
    }
  }, []);

  function onChrome(event: MouseEvent) {
    event.stopPropagation();
  }

  function runWindow(action: (current: ReturnType<typeof getCurrentWindow>) => void) {
    try {
      action(getCurrentWindow());
    } catch {
      /* vite preview has no Tauri window */
    }
  }

  return (
    <header
      className="titlebar"
      data-tauri-drag-region
      onDoubleClick={(event) => {
        if ((event.target as HTMLElement).closest("button")) return;
        runWindow((current) => void current.toggleMaximize());
      }}
    >
      <div className="titlebar-inner">
        <div className="titlebar-brand" data-tauri-drag-region>
          <svg
            className="titlebar-logo"
            viewBox="0 0 80 80"
            width={32}
            height={32}
            aria-hidden="true"
          >
            <rect x="4" y="4" width="72" height="72" rx="16" fill="#ffffff" />
            <g transform="translate(7.2 7.2) scale(.82)">
              <path
                fill="currentColor"
                fillRule="evenodd"
                d="M26 11 40 2l14 9v27H16V26h10V11Zm8 5 6-5 6 5v10H34V16Z"
              />
              <path fill="currentColor" d="M28 38h24l1.176471 10H26.823529L28 38Z" />
              <path fill="currentColor" d="M26.823529 48H35v12h-9.588235l1.411764-12Z" />
              <path fill="currentColor" d="M45 48h8.176471l1.411764 12H45V48Z" />
              <path fill="currentColor" d="M25.411765 60h29.17647L56 72H24l1.411765-12Z" />
              <path fill="currentColor" d="M73 16 57 29l16 13 6-6-10-7 10-7-6-6Z" />
            </g>
          </svg>
          <span>NAS Lighthouse</span>
        </div>
        <div className="titlebar-drag" data-tauri-drag-region />
        <div className="titlebar-controls no-drag">
          <button
            type="button"
            className="titlebar-btn no-drag"
            title="最小化"
            aria-label="最小化"
            onMouseDown={onChrome}
            onClick={() => runWindow((current) => void current.minimize())}
          >
            <Minus size={14} strokeWidth={2.2} />
          </button>
          <button
            type="button"
            className="titlebar-btn no-drag"
            title={maximized ? "还原" : "最大化"}
            aria-label={maximized ? "还原" : "最大化"}
            onMouseDown={onChrome}
            onClick={() => runWindow((current) => void current.toggleMaximize())}
          >
            {maximized ? <Copy size={12} strokeWidth={2.2} /> : <Square size={12} strokeWidth={2.2} />}
          </button>
          <button
            type="button"
            className="titlebar-btn titlebar-close no-drag"
            title="关闭"
            aria-label="关闭"
            onMouseDown={onChrome}
            onClick={() => runWindow((current) => void current.close())}
          >
            <X size={14} strokeWidth={2.2} />
          </button>
        </div>
      </div>
    </header>
  );
}
