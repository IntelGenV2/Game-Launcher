import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { AppSettings } from "../types";
import { useEffect, useState } from "react";

interface Props {
  open: boolean;
  settings: AppSettings;
  dataPath: string;
  onClose: () => void;
  onSave: (settings: AppSettings) => Promise<void>;
}

type UpdateStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; percent: number | null }
  | { kind: "restarting" }
  | { kind: "error"; message: string };

const DOWNLOAD_PAGE = "https://github.com/IntelGenV2/Game-Launcher/releases/latest";

export function SettingsModal({ open, settings, dataPath, onClose, onSave }: Props) {
  const [apiKey, setApiKey] = useState(settings.steamGridDbApiKey ?? "");
  const [saving, setSaving] = useState(false);
  const [appVersion, setAppVersion] = useState<string>("…");
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ kind: "idle" });
  const [pendingUpdate, setPendingUpdate] = useState<Awaited<ReturnType<typeof check>>>(null);

  useEffect(() => {
    if (open) {
      setApiKey(settings.steamGridDbApiKey ?? "");
      setUpdateStatus({ kind: "idle" });
      setPendingUpdate(null);
      getVersion()
        .then(setAppVersion)
        .catch(() => setAppVersion("unknown"));
    }
  }, [open, settings.steamGridDbApiKey]);

  if (!open) return null;

  async function handleCheckUpdate() {
    setUpdateStatus({ kind: "checking" });
    setPendingUpdate(null);
    try {
      const update = await check();
      if (!update) {
        setUpdateStatus({ kind: "upToDate" });
        return;
      }
      setPendingUpdate(update);
      setUpdateStatus({
        kind: "available",
        version: update.version,
        notes: update.body ?? null,
      });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setUpdateStatus({
        kind: "error",
        message:
          message.includes("Could not fetch") || message.includes("error sending request")
            ? "Could not reach GitHub Releases. Publish a release with latest.json (see UPDATES.md)."
            : message,
      });
    }
  }

  async function handleInstallUpdate() {
    if (!pendingUpdate) return;
    setUpdateStatus({ kind: "downloading", percent: 0 });
    try {
      let downloaded = 0;
      let total: number | null = null;
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null;
          setUpdateStatus({ kind: "downloading", percent: total ? 0 : null });
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total && total > 0) {
            setUpdateStatus({
              kind: "downloading",
              percent: Math.min(99, Math.round((downloaded / total) * 100)),
            });
          }
        } else if (event.event === "Finished") {
          setUpdateStatus({ kind: "downloading", percent: 100 });
        }
      });
      setUpdateStatus({ kind: "restarting" });
      await relaunch();
    } catch (e) {
      setUpdateStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }

  const busy =
    updateStatus.kind === "checking" ||
    updateStatus.kind === "downloading" ||
    updateStatus.kind === "restarting";

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
        <p>
          Covers for Steam work automatically. Add a SteamGridDB key for Epic/Xbox/EA/etc., or set
          cover art per game (copied into app storage).
        </p>

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
          <span className="hint">Free key at steamgriddb.com/profile/preferences/api</span>
        </div>

        <div className="field">
          <label>Library data folder</label>
          <input type="text" readOnly value={dataPath || "—"} />
        </div>

        <div className="field update-field">
          <label>Launcher updates</label>
          <div className="update-row">
            <span className="update-version">Installed version {appVersion}</span>
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => void handleCheckUpdate()}
            >
              {updateStatus.kind === "checking" ? "Checking…" : "Check for updates"}
            </button>
          </div>
          {updateStatus.kind === "upToDate" && (
            <span className="hint update-ok">You’re on the latest version.</span>
          )}
          {updateStatus.kind === "available" && (
            <div className="update-available">
              <span className="hint update-ok">
                Version {updateStatus.version} is available
                {updateStatus.notes ? ` — ${updateStatus.notes}` : ""}.
              </span>
              <button
                type="button"
                className="btn btn-primary"
                disabled={busy}
                onClick={() => void handleInstallUpdate()}
              >
                Download &amp; install
              </button>
            </div>
          )}
          {updateStatus.kind === "downloading" && (
            <span className="hint">
              Downloading update
              {updateStatus.percent != null ? ` (${updateStatus.percent}%)` : "…"}
            </span>
          )}
          {updateStatus.kind === "restarting" && (
            <span className="hint">Update installed — restarting…</span>
          )}
          {updateStatus.kind === "error" && (
            <span className="hint update-error">{updateStatus.message}</span>
          )}
          <button
            type="button"
            className="linkish"
            onClick={() => void openUrl(DOWNLOAD_PAGE)}
          >
            Open latest release on GitHub
          </button>
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
