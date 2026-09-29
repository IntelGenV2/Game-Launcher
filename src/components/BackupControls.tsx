import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

interface Preview { gameCount: number; artCount: number; createdAt: string | null; sizeBytes: number }
export function BackupControls() {
  const [preview, setPreview] = useState<(Preview & { path: string }) | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [prepared, setPrepared] = useState(false);
  async function choose() {
    setBusy(true); setMessage(null); setPreview(null);
    try {
      const path = await open({ multiple: false, filters: [{ name: "IntelGen library backup", extensions: ["zip"] }] });
      if (typeof path === "string") setPreview({ ...await invoke<Preview>("preview_backup", { path }), path });
    } catch (error) { setMessage(String(error)); }
    finally { setBusy(false); }
  }
  async function restore() {
    if (!preview) return;
    setBusy(true); setMessage(null);
    try {
      const safety = await invoke<string>("restore_backup", { path: preview.path });
      setPrepared(true);
      setMessage("Restore ready. Your current library was backed up to " + safety + ". Restart to finish.");
    } catch (error) { setMessage(String(error)); }
    finally { setBusy(false); }
  }
  return <div className="field backup-controls">
    <label>Restore a library backup</label>
    <p className="hint">Includes library entries, groups, pins, notes, history and artwork. Game saves and provider API keys are not included.</p>
    <div className="path-row">
      <button className="btn" disabled={busy || prepared} onClick={() => void choose()}>{busy ? "Working…" : "Choose backup zip"}</button>
      <button className="btn" onClick={() => void invoke("open_backups").catch(error => setMessage(String(error)))}>Open backup folder</button>
    </div>
    {preview && !prepared && <div className="restore-preview">
      <strong>Replace the current library?</strong>
      <p>{preview.gameCount} game entries · {preview.artCount} artwork files · {(preview.sizeBytes / 1024 / 1024).toFixed(1)} MB</p>
      {preview.createdAt && <p>Created {new Date(preview.createdAt).toLocaleString()}</p>}
      <p className="hint">A recovery backup is created first. The launcher must restart. Finish any running games before restoring.</p>
      <div className="path-row"><button className="btn btn-primary" disabled={busy} onClick={() => void restore()}>Prepare restore</button><button className="btn" disabled={busy} onClick={() => setPreview(null)}>Cancel</button></div>
    </div>}
    {message && <p className="hint" role="status">{message}</p>}
    {prepared && <button className="btn btn-primary" onClick={() => void relaunch().catch(error => setMessage("Restart the launcher manually to finish: " + String(error)))}>Restart and restore</button>}
  </div>;
}
