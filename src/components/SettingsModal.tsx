import { AppSettings } from "../types";
import { useEffect, useState } from "react";

interface Props {
  open: boolean;
  settings: AppSettings;
  dataPath: string;
  onClose: () => void;
  onSave: (settings: AppSettings) => Promise<void>;
}

export function SettingsModal({ open, settings, dataPath, onClose, onSave }: Props) {
  const [apiKey, setApiKey] = useState(settings.steamGridDbApiKey ?? "");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) setApiKey(settings.steamGridDbApiKey ?? "");
  }, [open, settings.steamGridDbApiKey]);

  if (!open) return null;

  return (
    <div className="settings-backdrop" onClick={onClose}>
      <div
        className="settings-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
      >
        <h2>Settings</h2>
        <p>Covers for Steam work automatically. Add a SteamGridDB key for Epic/Xbox/EA/etc., or set cover art per game (copied into app storage).</p>

        <div className="field">
          <label htmlFor="sgdb">SteamGridDB API key</label>
          <input
            id="sgdb"
            type="password"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            placeholder="Optional"
            autoComplete="off"
          />
          <span className="hint">
            Free key at steamgriddb.com/profile/preferences/api
          </span>
        </div>

        <div className="field">
          <label>Library data folder</label>
          <input type="text" readOnly value={dataPath || "—"} />
        </div>

        <div className="settings-actions">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving}
            onClick={async () => {
              setSaving(true);
              try {
                await onSave({
                  ...settings,
                  steamGridDbApiKey: apiKey.trim() || null,
                });
                onClose();
              } finally {
                setSaving(false);
              }
            }}
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
