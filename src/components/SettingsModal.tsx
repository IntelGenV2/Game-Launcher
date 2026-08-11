import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { AppSettings, isThemeId, THEME_OPTIONS, ThemeId } from "../types";
import { useEffect, useState } from "react";

interface Props {
  open: boolean;
  settings: AppSettings;
  dataPath: string;
  onClose: () => void;
  onSave: (settings: AppSettings) => Promise<void>;
  onPreviewAppearance: (theme: ThemeId, cardScale: number) => void;
}

type Section = "appearance" | "library" | "covers" | "updates";

type UpdateStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "downloading"; percent: number | null }
  | { kind: "restarting" }
  | { kind: "error"; message: string };

const DOWNLOAD_PAGE = "https://github.com/IntelGenV2/Game-Launcher/releases/latest";

const SECTIONS: { id: Section; label: string }[] = [
  { id: "appearance", label: "Appearance" },
  { id: "library", label: "Library" },
  { id: "covers", label: "Covers" },
  { id: "updates", label: "Updates" },
];

export function SettingsModal({
  open,
  settings,
  dataPath,
  onClose,
  onSave,
  onPreviewAppearance,
}: Props) {
  const [section, setSection] = useState<Section>("appearance");
  const [apiKey, setApiKey] = useState(settings.steamGridDbApiKey ?? "");
  const [theme, setTheme] = useState<ThemeId>(
    isThemeId(settings.theme) ? settings.theme : "emerald",
  );
  const [cardScale, setCardScale] = useState(settings.cardScale ?? 1);
  const [saving, setSaving] = useState(false);
  const [appVersion, setAppVersion] = useState<string>("…");
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ kind: "idle" });
  const [pendingUpdate, setPendingUpdate] = useState<Awaited<ReturnType<typeof check>>>(null);

  useEffect(() => {
    if (open) {
      setApiKey(settings.steamGridDbApiKey ?? "");
      setTheme(isThemeId(settings.theme) ? settings.theme : "emerald");
      setCardScale(typeof settings.cardScale === "number" ? settings.cardScale : 1);
      setUpdateStatus({ kind: "idle" });
      setPendingUpdate(null);
      setSection("appearance");
      getVersion()
        .then(setAppVersion)
        .catch(() => setAppVersion("unknown"));
    }
  }, [open, settings]);

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

  function previewTheme(next: ThemeId) {
    setTheme(next);
    onPreviewAppearance(next, cardScale);
  }

  function previewScale(next: number) {
    setCardScale(next);
    onPreviewAppearance(theme, next);
  }

  return (
    <div
      className="settings-backdrop"
      onClick={() => {
        onPreviewAppearance(
          isThemeId(settings.theme) ? settings.theme : "emerald",
          typeof settings.cardScale === "number" ? settings.cardScale : 1,
        );
        onClose();
      }}
    >
      <div
        className="settings-panel settings-panel-wide"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
      >
        <h2>Settings</h2>
        <div className="settings-layout">
          <nav className="settings-nav" aria-label="Settings sections">
            {SECTIONS.map((s) => (
              <button
                key={s.id}
                type="button"
                className={`settings-nav-btn${section === s.id ? " active" : ""}`}
                onClick={() => setSection(s.id)}
              >
                {s.label}
              </button>
            ))}
          </nav>

          <div className="settings-body">
            {section === "appearance" && (
              <>
                <p className="settings-lead">Theme colors and cover card size.</p>
                <div className="field">
                  <label>Theme</label>
                  <div className="theme-grid">
                    {THEME_OPTIONS.map((t) => (
                      <button
                        key={t.id}
                        type="button"
                        className={`theme-swatch${theme === t.id ? " active" : ""}`}
                        data-theme-preview={t.id}
                        onClick={() => previewTheme(t.id)}
                      >
                        <span className="theme-dot" />
                        {t.label}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="field field-scale-row">
                  <label htmlFor="card-scale">
                    Card scale <span className="hint-inline">{cardScale.toFixed(2)}×</span>
                  </label>
                  <input
                    id="card-scale"
                    type="range"
                    min={0.7}
                    max={1.4}
                    step={0.05}
                    value={cardScale}
                    onChange={(e) => previewScale(Number(e.target.value))}
                  />
                </div>
              </>
            )}

            {section === "library" && (
              <>
                <p className="settings-lead">Where library data and cached covers are stored.</p>
                <div className="field">
                  <label>Library data folder</label>
                  <input type="text" readOnly value={dataPath || "—"} />
                </div>
              </>
            )}

            {section === "covers" && (
              <>
                <p className="settings-lead">
                  Steam covers work automatically. Add a SteamGridDB key for better Epic/Xbox/EA
                  matches, or set cover art per game.
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
              </>
            )}

            {section === "updates" && (
              <>
                <p className="settings-lead">Check GitHub Releases for a newer launcher build.</p>
                <div className="field update-field">
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
              </>
            )}
          </div>
        </div>

        <div className="settings-actions">
          <button
            type="button"
            className="btn"
            onClick={() => {
              onPreviewAppearance(
                isThemeId(settings.theme) ? settings.theme : "emerald",
                typeof settings.cardScale === "number" ? settings.cardScale : 1,
              );
              onClose();
            }}
          >
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
                  theme,
                  cardScale,
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
