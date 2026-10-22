export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className={`toggle${checked ? " is-on" : ""}`}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-knob">{checked ? "开" : "关"}</span>
    </button>
  );
}

export function ToggleRow({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <div className={`toggle-row${disabled ? " is-disabled" : ""}`}>
      <div className="toggle-copy">
        <p>{label}</p>
        {hint ? <small>{hint}</small> : null}
      </div>
      <Toggle checked={checked} onChange={onChange} label={label} disabled={disabled} />
    </div>
  );
}
