import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { ChevronDown } from "lucide-react";

const CLOSE_MS = 240;

export type GlideOption = { value: string; label: string };

export function GlideSelect({
  value,
  options,
  onChange,
  disabled,
  className,
  "aria-label": ariaLabel,
}: {
  value: string;
  options: GlideOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
}) {
  const [open, setOpen] = useState(false);
  const [height, setHeight] = useState<number>();
  const [cursor, setCursor] = useState({ top: 0, height: 36, visible: false, index: -1 });
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const listboxId = useId();

  const selected = options.find((option) => option.value === value) ?? options[0];
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));

  useLayoutEffect(() => {
    if (!open) return;
    const node = listRef.current;
    if (node) setHeight(node.offsetHeight);
  }, [open, options]);

  useEffect(() => {
    if (open) return;
    const timer = window.setTimeout(() => setCursor((state) => ({ ...state, visible: false })), CLOSE_MS);
    return () => window.clearTimeout(timer);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const rows = listRef.current?.querySelectorAll<HTMLButtonElement>("[data-row]");
    const current = rows?.[selectedIndex];
    if (current) {
      setCursor({
        top: current.offsetTop,
        height: current.offsetHeight,
        visible: true,
        index: selectedIndex,
      });
      current.focus({ preventScroll: true });
    }
  }, [open, selectedIndex]);

  function dismiss() {
    setOpen(false);
    triggerRef.current?.focus({ preventScroll: true });
  }

  function choose(next: string) {
    onChange(next);
    dismiss();
  }

  function moveCursor(row: HTMLButtonElement, index: number) {
    setCursor({
      top: row.offsetTop,
      height: row.offsetHeight,
      visible: true,
      index,
    });
  }

  function moveFocus(delta: number) {
    const rows = Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>("[data-row]") ?? []);
    if (!rows.length) return;
    const index = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = index < 0 ? (delta > 0 ? 0 : rows.length - 1) : (index + delta + rows.length) % rows.length;
    rows[next].focus({ preventScroll: true });
    moveCursor(rows[next], next);
  }

  function onRootKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      moveFocus(event.key === "ArrowDown" ? 1 : -1);
      return;
    }
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      dismiss();
    }
  }

  return (
    <div
      ref={rootRef}
      className={`glide-select${open ? " is-open" : ""}${className ? ` ${className}` : ""}`}
      onKeyDown={onRootKeyDown}
    >
      <button
        ref={triggerRef}
        type="button"
        className="glide-select-trigger"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listboxId}
        aria-label={ariaLabel}
        onClick={() => (open ? dismiss() : setOpen(true))}
      >
        <span>{selected?.label ?? ""}</span>
        <ChevronDown size={16} strokeWidth={1.8} aria-hidden="true" />
      </button>
      <div className="glide-select-menu" inert={open ? undefined : true}>
        <div className="glide-select-clip" style={height != null ? { height } : undefined}>
          <div
            ref={listRef}
            className="glide-select-list"
            id={listboxId}
            role="listbox"
            aria-activedescendant={cursor.index >= 0 ? `${listboxId}-${options[cursor.index]?.value}` : undefined}
            onMouseLeave={() => setCursor((state) => ({ ...state, visible: false }))}
          >
            <span
              className={`glide-select-cursor${cursor.visible ? " is-on" : ""}`}
              style={{ transform: `translateY(${cursor.top}px)`, height: cursor.height }}
              aria-hidden="true"
            />
            {options.map((option, index) => (
              <button
                key={option.value}
                id={`${listboxId}-${option.value}`}
                type="button"
                role="option"
                data-row=""
                className="glide-select-option"
                aria-selected={option.value === value}
                onMouseEnter={(event) => moveCursor(event.currentTarget, index)}
                onFocus={(event) => moveCursor(event.currentTarget, index)}
                onClick={() => choose(option.value)}
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
