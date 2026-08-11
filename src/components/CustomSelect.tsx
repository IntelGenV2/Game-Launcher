import { useEffect, useId, useRef, useState } from "react";

export interface SelectOption<T extends string> {
  id: T;
  label: string;
}

interface Props<T extends string> {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
}

export function CustomSelect<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className = "",
}: Props<T>) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const selected = options.find((o) => o.id === value) ?? options[0];

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className={`custom-select${open ? " open" : ""} ${className}`.trim()} ref={rootRef}>
      <button
        type="button"
        className="custom-select-trigger"
        aria-label={ariaLabel}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        onClick={() => setOpen((v) => !v)}
      >
        <span>{selected?.label ?? ""}</span>
        <span className="custom-select-caret" aria-hidden>
          ▾
        </span>
      </button>
      {open && (
        <ul className="custom-select-menu" role="listbox" id={listId} aria-label={ariaLabel}>
          {options.map((o) => (
            <li key={o.id} role="presentation">
              <button
                type="button"
                role="option"
                aria-selected={o.id === value}
                className={`custom-select-option${o.id === value ? " active" : ""}`}
                onClick={() => {
                  onChange(o.id);
                  setOpen(false);
                }}
              >
                {o.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
