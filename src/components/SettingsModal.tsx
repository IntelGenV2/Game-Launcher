import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import {
  AppSettings,
  AppearancePrefs,
  CORNER_OPTIONS,
  CoverCorners,
  CoverShape,
  DENSITY_OPTIONS,
  GridDensity,
  SHAPE_OPTIONS,
  THEME_OPTIONS,
  ThemeId,
  appearanceFromSettings,
} from "../types";
import { useEffect, useState } from "react";

interface Props {
  open: boolean;
  settings: AppSettings;
  onClose: () => void;
  onSave: (settings: AppSettings) => Promise<void>;
  onPreviewAppearance: (prefs: AppearancePrefs) => void;
  onResetArt: () => Promise<void>;
  onResetStats: () => Promise<void>;
}

type Section = "appearance" | "library" | "covers" | "system" | "updates" | "reset";

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
  { id: "system", label: "System" },
  { id: "updates", label: "Updates" },
  { id: "reset", label: "Reset" },
];

export function SettingsModal({
  open,
  settings,
  onClose,
  onSave,
  onPreviewAppearance,
  onResetArt,
  onResetStats,
}: Props) {
  const [section, setSection] = useState<Section>("appearance");
  const [apiKey, setApiKey] = useState(settings.steamGridDbApiKey ?? "");
  const [theme, setTheme] = useState<ThemeId>("emerald");
  const [cardScale, setCardScale] = useState(1);
  const [showTitles, setShowTitles] = useState(true);
  const [showStoreLabels, setShowStoreLabels] = useState(true);
  const [gridDensity, setGridDensity] = useState<GridDensity>("normal");
  const [coverCorners, setCoverCorners] = useState<CoverCorners>("soft");
  const [coverShape, setCoverShape] = useState<CoverShape>("portrait");
  const [reduceMotion, setReduceMotion] = useState(false);
  const [startWithWindows, setStartWithWindows] = useState(false);
  const [closeToTray, setCloseToTray] = useState(false);
  const [startInBackground, setStartInBackground] = useState(false);
  const [saving, setSaving] = useState(false);
  const [appVersion, setAppVersion] = useState<string>("…");
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>({ kind: "idle" });
  const [pendingUpdate, setPendingUpdate] = useState<Awaited<ReturnType<typeof check>>>(null);
  const [sysBusy, setSysBusy] = useState<string | null>(null);
  const [sysMsg, setSysMsg] = useState<string | null>(null);

  function currentPrefs(): AppearancePrefs {
    return {
      theme,
      cardScale,
      showTitles,
      showStoreLabels,
      gridDensity,
      coverCorners,
      coverShape,
      reduceMotion,
    };
  }

  function loadFromSettings(s: AppSettings) {
    const a = appearanceFromSettings(s);
    setApiKey(s.steamGridDbApiKey ?? "");
    setTheme(a.theme);
    setCardScale(a.cardScale);
    setShowTitles(a.showTitles !== false);
    setShowStoreLabels(a.showStoreLabels !== false);
    setGridDensity(a.gridDensity ?? "normal");
    setCoverCorners(a.coverCorners ?? "soft");
    setCoverShape(a.coverShape ?? "portrait");
    setReduceMotion(a.reduceMotion === true);
    setStartWithWindows(s.startWithWindows === true);
    setCloseToTray(s.closeToTray === true);
    setStartInBackground(s.startInBackground === true);
    setSysMsg(null);
  }

  useEffect(() => {
    if (open) {
      loadFromSettings(settings);
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
            ? "Could not reach GitHub Releases. Check your connection, or try again later."
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

  function preview(patch: Partial<AppearancePrefs>) {
    const next = { ...currentPrefs(), ...patch };
    if (patch.theme) setTheme(patch.theme);
    if (patch.cardScale != null) setCardScale(patch.cardScale);
    if (patch.showTitles != null) setShowTitles(patch.showTitles);
    if (patch.showStoreLabels != null) setShowStoreLabels(patch.showStoreLabels);
    if (patch.gridDensity) setGridDensity(patch.gridDensity);
    if (patch.coverCorners) setCoverCorners(patch.coverCorners);
    if (patch.coverShape) setCoverShape(patch.coverShape);
    if (patch.reduceMotion != null) setReduceMotion(patch.reduceMotion);
    onPreviewAppearance(next);
  }

  function cancelAndClose() {
    onPreviewAppearance(appearanceFromSettings(settings));
    onClose();
  }

  return (
    <div className="settings-backdrop" onClick={cancelAndClose}>
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
                <div className="field">
                  <label>Theme</label>
                  <div className="theme-grid">
                    {THEME_OPTIONS.map((t) => (
                      <button
                        key={t.id}
                        type="button"
                        className={`theme-swatch${theme === t.id ? " active" : ""}`}
                        data-theme-preview={t.id}
                        onClick={() => preview({ theme: t.id })}
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
                    onChange={(e) => preview({ cardScale: Number(e.target.value) })}
                  />
                </div>
                <div className="field">
                  <label>Grid density</label>
                  <div className="chip-row">
                    {DENSITY_OPTIONS.map((d) => (
                      <button
                        key={d.id}
                        type="button"
                        className={`chip-btn${gridDensity === d.id ? " active" : ""}`}
                        onClick={() => preview({ gridDensity: d.id })}
                      >
                        {d.label}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="field">
                  <label>Cover shape</label>
                  <div className="chip-row">
                    {SHAPE_OPTIONS.map((s) => (
                      <button
                        key={s.id}
                        type="button"
                        className={`chip-btn${coverShape === s.id ? " active" : ""}`}
                        onClick={() => preview({ coverShape: s.id })}
                      >
                        {s.label}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="field">
                  <label>Cover corners</label>
                  <div className="chip-row">
                    {CORNER_OPTIONS.map((c) => (
                      <button
                        key={c.id}
                        type="button"
                        className={`chip-btn${coverCorners === c.id ? " active" : ""}`}
                        onClick={() => preview({ coverCorners: c.id })}
                      >
                        {c.label}
                      </button>
                    ))}
                  </div>
                </div>
                <div className="field toggle-stack">
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={showTitles}
                      onChange={(e) => preview({ showTitles: e.target.checked })}
                    />
                    Show game titles
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={showStoreLabels}
                      onChange={(e) => preview({ showStoreLabels: e.target.checked })}
                    />
                    Show store labels
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={reduceMotion}
                      onChange={(e) => preview({ reduceMotion: e.target.checked })}
                    />
                    Reduce motion
                  </label>
                </div>
              </>
            )}

            {section === "library" && (
              <>
                <div className="field">
                  <label>Backup</label>
                  <button
                    type="button"
                    className="btn"
                    disabled={!!sysBusy}
                    onClick={async () => {
                      const dest = await save({
                        defaultPath: "IntelLauncher-backup.zip",
                        filters: [{ name: "Zip", extensions: ["zip"] }],
                      });
                      if (!dest || typeof dest !== "string") return;
                      setSysBusy("backup");
                      setSysMsg(null);
                      try {
                        await invoke("export_backup", { dest });
                        setSysMsg("Backup saved.");
                      } catch (e) {
                        setSysMsg(String(e));
                      } finally {
                        setSysBusy(null);
                      }
                    }}
                  >
                    {sysBusy === "backup" ? "Exporting…" : "Export library zip"}
                  </button>
                </div>
                <div className="field">
                  <label>Import from Playnite</label>
                  <div className="path-row">
                    <button
                      type="button"
                      className="btn"
                      disabled={!!sysBusy}
                      onClick={async () => {
                        const suggested = await invoke<string | null>("default_playnite_path").catch(() => null);
                        const picked = await openDialog({
                          directory: true,
                          defaultPath: suggested ?? undefined,
                          title: "Select Playnite data folder or export",
                        });
                        if (!picked || typeof picked !== "string") return;
                        setSysBusy("playnite");
                        setSysMsg(null);
                        try {
                          const r = await invoke<{ updated: number; added: number; skipped: number }>(
                            "import_playnite",
                            { path: picked },
                          );
                          setSysMsg(`Playnite import: ${r.updated} updated, ${r.added} added, ${r.skipped} skipped.`);
                        } catch (e) {
                          setSysMsg(String(e));
                        } finally {
                          setSysBusy(null);
                        }
                      }}
                    >
                      {sysBusy === "playnite" ? "Importing…" : "Choose Playnite folder"}
                    </button>
                    <button
                      type="button"
                      className="btn"
                      disabled={!!sysBusy}
                      onClick={async () => {
                        const picked = await openDialog({
                          multiple: false,
                          filters: [{ name: "Playnite export", extensions: ["yaml", "yml", "json"] }],
                        });
                        if (!picked || typeof picked !== "string") return;
                        setSysBusy("playnite");
                        setSysMsg(null);
                        try {
                          const r = await invoke<{ updated: number; added: number; skipped: number }>(
                            "import_playnite",
                            { path: picked },
                          );
                          setSysMsg(`Playnite import: ${r.updated} updated, ${r.added} added, ${r.skipped} skipped.`);
                        } catch (e) {
                          setSysMsg(String(e));
                        } finally {
                          setSysBusy(null);
                        }
                      }}
                    >
                      YAML / JSON file
                    </button>
                  </div>
                </div>
                {sysMsg && <p className="hint">{sysMsg}</p>}
              </>
            )}

            {section === "covers" && (
              <>
                <p className="settings-lead">
                  Add a SteamGridDB key for better Epic/Xbox/EA matches.
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

            {section === "system" && (
              <>
                <p className="settings-lead">Startup and tray</p>
                <div className="field toggle-stack">
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={startWithWindows}
                      onChange={(e) => {
                        const on = e.target.checked;
                        setStartWithWindows(on);
                        if (on) {
                          setStartInBackground(true);
                          setCloseToTray(true);
                        }
                      }}
                    />
                    Start with Windows
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={startInBackground}
                      onChange={(e) => {
                        const on = e.target.checked;
                        setStartInBackground(on);
                        if (on) setCloseToTray(true);
                      }}
                    />
                    Start in background (tray)
                  </label>
                  <label className="toggle-row">
                    <input
                      type="checkbox"
                      checked={closeToTray}
                      onChange={(e) => setCloseToTray(e.target.checked)}
                    />
                    Close to tray instead of quitting
                  </label>
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

            {section === "reset" && (
              <>
                <div className="field">
                  <label>Stats</label>
                  <button
                    type="button"
                    className="btn"
                    disabled={!!sysBusy}
                    onClick={async () => {
                      if (
                        !window.confirm(
                          "Reset all playtime, last played, and session history? Your games stay in the library.",
                        )
                      ) {
                        return;
                      }
                      setSysBusy("reset-stats");
                      setSysMsg(null);
                      try {
                        await onResetStats();
                        setSysMsg("Playtime and sessions cleared.");
                      } catch (e) {
                        setSysMsg(String(e));
                      } finally {
                        setSysBusy(null);
                      }
                    }}
                  >
                    {sysBusy === "reset-stats" ? "Resetting…" : "Reset stats"}
                  </button>
                </div>
                <div className="field">
                  <label>Cover art</label>
                  <button
                    type="button"
                    className="btn"
                    disabled={!!sysBusy}
                    onClick={async () => {
                      if (
                        !window.confirm(
                          "Delete all downloaded and custom cover art, then fetch covers again?",
                        )
                      ) {
                        return;
                      }
                      setSysBusy("reset-art");
                      setSysMsg(null);
                      try {
                        await onResetArt();
                        setSysMsg("Cover art reset. Fetching replacements…");
                      } catch (e) {
                        setSysMsg(String(e));
                      } finally {
                        setSysBusy(null);
                      }
                    }}
                  >
                    {sysBusy === "reset-art" ? "Resetting…" : "Reset all art"}
                  </button>
                </div>
                <div className="field">
                  <label>Whole app</label>
                  <button
                    type="button"
                    className="btn btn-danger"
                    disabled={!!sysBusy}
                    onClick={async () => {
                      if (
                        !window.confirm(
                          "Reset the app? This deletes your library, settings, groups, and covers. The launcher will restart empty.",
                        )
                      ) {
                        return;
                      }
                      setSysBusy("reset-app");
                      try {
                        await invoke("reset_app");
                        await relaunch();
                      } catch (e) {
                        setSysBusy(null);
                        setSysMsg(String(e));
                      }
                    }}
                  >
                    {sysBusy === "reset-app" ? "Restarting…" : "Reset app"}
                  </button>
                </div>
                {sysMsg && <p className="hint">{sysMsg}</p>}
              </>
            )}
          </div>
        </div>

        <div className="settings-actions">
          <button type="button" className="btn" onClick={cancelAndClose}>
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
                  showTitles,
                  showStoreLabels,
                  gridDensity,
                  coverCorners,
                  coverShape,
                  reduceMotion,
                  startWithWindows,
                  closeToTray,
                  startInBackground,
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
