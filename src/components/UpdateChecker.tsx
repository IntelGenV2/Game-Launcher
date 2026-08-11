import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useRef, useState } from "react";

const SNOOZE_KEY = "intelgen.update.snooze";
const CHECK_DELAY_MS = 8_000;

type Phase =
  | { kind: "hidden" }
  | { kind: "available"; version: string }
  | { kind: "installing"; percent: number | null }
  | { kind: "restarting" };

function readSnooze(): string | null {
  try {
    return localStorage.getItem(SNOOZE_KEY);
  } catch {
    return null;
  }
}

function writeSnooze(version: string) {
  try {
    localStorage.setItem(SNOOZE_KEY, version);
  } catch {
    /* ignore */
  }
}

function clearSnooze() {
  try {
    localStorage.removeItem(SNOOZE_KEY);
  } catch {
    /* ignore */
  }
}

/** Quiet background update check — only surfaces UI when a newer build exists. */
export function UpdateChecker() {
  const [phase, setPhase] = useState<Phase>({ kind: "hidden" });
  const updateRef = useRef<Update | null>(null);

  useEffect(() => {
    let cancelled = false;
    const timer = window.setTimeout(async () => {
      try {
        const update = await check();
        if (cancelled || !update) return;
        if (readSnooze() === update.version) return;
        updateRef.current = update;
        setPhase({ kind: "available", version: update.version });
      } catch {
        /* silent — offline / no release feed */
      }
    }, CHECK_DELAY_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, []);

  async function handleInstall() {
    const update = updateRef.current;
    if (!update) return;
    setPhase({ kind: "installing", percent: 0 });
    try {
      let downloaded = 0;
      let total: number | null = null;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null;
          setPhase({ kind: "installing", percent: total ? 0 : null });
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total && total > 0) {
            setPhase({
              kind: "installing",
              percent: Math.min(99, Math.round((downloaded / total) * 100)),
            });
          }
        } else if (event.event === "Finished") {
          setPhase({ kind: "installing", percent: 100 });
        }
      });
      clearSnooze();
      setPhase({ kind: "restarting" });
      await relaunch();
    } catch {
      setPhase({ kind: "available", version: update.version });
    }
  }

  function handleLater() {
    const version =
      phase.kind === "available"
        ? phase.version
        : updateRef.current?.version;
    if (version) writeSnooze(version);
    updateRef.current = null;
    setPhase({ kind: "hidden" });
  }

  if (phase.kind === "hidden") return null;

  return (
    <div className="update-chip" role="status">
      {phase.kind === "available" && (
        <>
          <span className="update-chip-text">Update {phase.version} available</span>
          <button type="button" className="update-chip-btn" onClick={() => void handleInstall()}>
            Install
          </button>
          <button type="button" className="update-chip-btn quiet" onClick={handleLater}>
            Later
          </button>
        </>
      )}
      {phase.kind === "installing" && (
        <span className="update-chip-text">
          Downloading
          {phase.percent != null ? ` ${phase.percent}%` : "…"}
        </span>
      )}
      {phase.kind === "restarting" && (
        <span className="update-chip-text">Restarting…</span>
      )}
    </div>
  );
}
