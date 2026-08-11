import { useEffect, useRef, useState } from "react";

interface Props {
  open: boolean;
  title: string;
  initialName?: string;
  confirmLabel?: string;
  onClose: () => void;
  onConfirm: (name: string) => void | Promise<void>;
}

export function GroupNameModal({
  open,
  title,
  initialName = "New group",
  confirmLabel = "Create",
  onClose,
  onConfirm,
}: Props) {
  const [name, setName] = useState(initialName);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setName(initialName);
    setBusy(false);
    const t = window.setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 30);
    return () => window.clearTimeout(t);
  }, [open, initialName]);

  if (!open) return null;

  async function submit() {
    const next = name.trim();
    if (!next || busy) return;
    setBusy(true);
    try {
      await onConfirm(next);
      onClose();
    } catch {
      setBusy(false);
    }
  }

  return (
    <div
      className="settings-backdrop"
      onClick={() => !busy && onClose()}
      role="presentation"
    >
      <div
        className="settings-panel group-name-panel"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <h2>{title}</h2>
        <p className="settings-lead">Name your collection. You can add games next.</p>
        <div className="field">
          <label htmlFor="group-name-input">Group name</label>
          <input
            id="group-name-input"
            ref={inputRef}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submit();
              if (e.key === "Escape") onClose();
            }}
            placeholder="e.g. Co-op"
            autoComplete="off"
          />
        </div>
        <div className="settings-actions">
          <button type="button" className="btn" disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy || !name.trim()}
            onClick={() => void submit()}
          >
            {busy ? "Saving…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
