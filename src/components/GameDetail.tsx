import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import {
  Game,
  GameStats,
  STORE_LABELS,
  Store,
  coverSrc,
  formatLastPlayed,
  formatPlaytime,
} from "../types";
import { BarChart, LineChart } from "./Charts";

interface Props {
  game: Game;
  coverDataUrl?: string | null;
  liveFps?: number | null;
  onBack: () => void;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onRemove: (game: Game) => void;
  onGameUpdated: (game: Game) => void;
  onCoverUpdated?: (id: string, dataUrl: string) => void;
}

type EditPanel = "menu" | "name" | null;

function pathLabelFor(game: Game): string {
  const lt = game.launchTarget?.trim() ?? "";
  const looksLikeFs =
    lt.includes("\\") || lt.includes("/") || lt.toLowerCase().endsWith(".exe");
  if (looksLikeFs) return lt;
  if (game.installPath?.trim()) return game.installPath.trim();
  if (game.store === "steam" && /^\d+$/.test(lt)) return `Steam App ${lt}`;
  return lt || "No path set";
}

function scrollDetailToTop() {
  const main = document.querySelector(".main");
  if (main) main.scrollTop = 0;
  window.scrollTo(0, 0);
}

export function GameDetail({
  game,
  coverDataUrl,
  liveFps = null,
  onBack,
  onLaunch,
  onToggleFavorite,
  onRemove,
  onGameUpdated,
  onCoverUpdated,
}: Props) {
  const [stats, setStats] = useState<GameStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [imgFailed, setImgFailed] = useState(false);
  const [editPanel, setEditPanel] = useState<EditPanel>(null);
  const [nameDraft, setNameDraft] = useState(game.name);
  const [busy, setBusy] = useState(false);
  const pageRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const src = coverSrc(game, coverDataUrl);
  const pathLabel = pathLabelFor(game);

  async function refreshStats() {
    try {
      const s = await invoke<GameStats>("get_game_stats", { id: game.id });
      setStats(s);
    } catch {
      setStats(null);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    setLoading(true);
    setImgFailed(false);
    setEditPanel(null);
    setNameDraft(game.name);
    refreshStats();

    // Scroll the library pane to top before paint and again after layout
    scrollDetailToTop();
    requestAnimationFrame(() => {
      scrollDetailToTop();
      requestAnimationFrame(scrollDetailToTop);
    });

    if (!game.coverPath) {
      invoke("fetch_covers", { ids: [game.id] }).catch(() => undefined);
    } else if (!coverDataUrl) {
      invoke<string | null>("get_cover_data_url", { id: game.id })
        .then((url) => {
          if (url) onCoverUpdated?.(game.id, url);
        })
        .catch(() => undefined);
    }
  }, [game.id]);

  useEffect(() => {
    setNameDraft(game.name);
  }, [game.name]);

  useEffect(() => {
    setImgFailed(false);
  }, [src]);

  useEffect(() => {
    if (editPanel === "name") {
      nameInputRef.current?.focus();
      nameInputRef.current?.select();
    }
  }, [editPanel]);

  async function handleSetCover() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_custom_cover", { id: game.id, path: selected });
      onGameUpdated(updated);
      setImgFailed(false);
      const dataUrl = await invoke<string | null>("get_cover_data_url", { id: game.id });
      if (dataUrl) onCoverUpdated?.(game.id, dataUrl);
      setEditPanel(null);
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleSetPath() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_game_path", { id: game.id, path: selected });
      onGameUpdated(updated);
      setEditPanel(null);
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function commitName() {
    const next = nameDraft.trim();
    if (!next) {
      alert("Name cannot be empty");
      return;
    }
    if (next === game.name) {
      setEditPanel(null);
      return;
    }
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_game_name", { id: game.id, name: next });
      onGameUpdated(updated);
      setEditPanel(null);
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  }

  const dailyPoints =
    stats?.dailyPlaytime.map((d) => ({
      label: d.day.slice(5),
      value: d.minutes,
    })) ?? [];

  const fpsPoints =
    stats?.fpsSamples.map((s) => ({
      label: new Date(s.recordedAt).toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
      }),
      value: s.fps,
    })) ??
    stats?.sessions
      .filter((s) => s.avgFps != null)
      .map((s) => ({
        label: new Date(s.startedAt).toLocaleDateString(undefined, {
          month: "short",
          day: "numeric",
        }),
        value: s.avgFps as number,
      })) ??
    [];

  return (
    <div className="detail-page" ref={pageRef}>
      <button type="button" className="btn back-btn" onClick={onBack}>
        ← Library
      </button>

      <div className="detail-hero">
        <div className="detail-cover">
          {src && !imgFailed ? (
            <img src={src} alt="" onError={() => setImgFailed(true)} />
          ) : (
            <div className="cover-fallback large">{game.name.charAt(0)}</div>
          )}
          <button
            type="button"
            className="play-btn detail-play"
            disabled={game.missing}
            onClick={() => onLaunch(game)}
          >
            ▶
          </button>
        </div>

        <div className="detail-info">
          <h1>{game.name}</h1>
          <p className="detail-store">
            {STORE_LABELS[game.store as Store] ?? game.store}
            {game.missing ? " · Missing from disk" : ""}
          </p>
          <p className="detail-path" title={pathLabel}>
            {pathLabel}
          </p>

          <div className="stat-grid">
            <div className="stat-pill">
              <span className="stat-label">Playtime</span>
              <span className="stat-value">{formatPlaytime(game.playtimeMinutes)}</span>
            </div>
            <div className="stat-pill">
              <span className="stat-label">Last played</span>
              <span className="stat-value">{formatLastPlayed(game.lastPlayedAt)}</span>
            </div>
            <div className="stat-pill">
              <span className="stat-label">Sessions</span>
              <span className="stat-value">{loading ? "…" : stats?.sessionCount ?? 0}</span>
            </div>
            <div className="stat-pill">
              <span className="stat-label">Live FPS</span>
              <span className={`stat-value${liveFps ? " live" : ""}`}>
                {liveFps != null && liveFps > 0 ? Math.round(liveFps) : "—"}
              </span>
            </div>
            <div className="stat-pill">
              <span className="stat-label">Avg FPS</span>
              <span className="stat-value">
                {stats?.avgFps != null ? Math.round(stats.avgFps) : "—"}
              </span>
            </div>
          </div>

          <div className="detail-actions">
            <button
              type="button"
              className="btn btn-primary"
              disabled={game.missing}
              onClick={() => onLaunch(game)}
            >
              ▶ Play
            </button>
            <button type="button" className="btn" onClick={() => onToggleFavorite(game)}>
              {game.favorite ? "★ Favorited" : "☆ Favorite"}
            </button>
            <button type="button" className="btn" onClick={() => setEditPanel("menu")}>
              ✎ Edit
            </button>
            <button
              type="button"
              className="btn btn-danger"
              onClick={() => {
                if (confirm(`Remove “${game.name}” from the launcher?`)) onRemove(game);
              }}
            >
              Remove from launcher
            </button>
          </div>
          <p className="hint">FPS is logged automatically while you play.</p>
        </div>
      </div>

      <div className="charts-row">
        <BarChart
          title="Playtime (last 14 days)"
          points={dailyPoints}
          formatValue={(v) => `${v}m`}
        />
        <LineChart
          title="FPS history"
          points={fpsPoints}
          emptyText="Play the game to build an automatic FPS chart."
        />
      </div>

      <div className="sessions-panel">
        <h3>Recent sessions</h3>
        {!stats || stats.sessions.length === 0 ? (
          <p className="chart-empty">No sessions yet. Hit Play to start tracking.</p>
        ) : (
          <ul className="session-list">
            {[...stats.sessions]
              .reverse()
              .slice(0, 12)
              .map((s) => (
                <li key={s.id}>
                  <span>{new Date(s.startedAt).toLocaleString()}</span>
                  <span>{s.durationMinutes > 0 ? `${s.durationMinutes}m` : "in progress"}</span>
                  <span>{s.avgFps != null ? `${Math.round(s.avgFps)} FPS` : "—"}</span>
                </li>
              ))}
          </ul>
        )}
      </div>

      {editPanel && (
        <div className="settings-backdrop" onClick={() => !busy && setEditPanel(null)}>
          <div
            className="settings-panel edit-game-panel"
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-label="Edit game"
          >
            {editPanel === "menu" && (
              <>
                <h2>Edit {game.name}</h2>
                <p>Choose what you want to change.</p>
                <div className="edit-choice-list">
                  <button
                    type="button"
                    className="edit-choice"
                    disabled={busy}
                    onClick={() => void handleSetCover()}
                  >
                    <span className="edit-choice-title">Cover art</span>
                    <span className="edit-choice-desc">Pick an image file to use as the tile cover</span>
                  </button>
                  <button
                    type="button"
                    className="edit-choice"
                    disabled={busy}
                    onClick={() => {
                      setNameDraft(game.name);
                      setEditPanel("name");
                    }}
                  >
                    <span className="edit-choice-title">Display name</span>
                    <span className="edit-choice-desc">Rename how this game appears in your library</span>
                  </button>
                  <button
                    type="button"
                    className="edit-choice"
                    disabled={busy}
                    onClick={() => void handleSetPath()}
                  >
                    <span className="edit-choice-title">Game path</span>
                    <span className="edit-choice-desc">Point at a different .exe (kept after refresh)</span>
                  </button>
                </div>
                <div className="settings-actions">
                  <button type="button" className="btn" disabled={busy} onClick={() => setEditPanel(null)}>
                    Cancel
                  </button>
                </div>
              </>
            )}

            {editPanel === "name" && (
              <>
                <h2>Display name</h2>
                <p>This name stays even when you rescan your library.</p>
                <div className="field">
                  <label htmlFor="game-display-name">Name</label>
                  <input
                    id="game-display-name"
                    ref={nameInputRef}
                    value={nameDraft}
                    maxLength={120}
                    disabled={busy}
                    onChange={(e) => setNameDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void commitName();
                      } else if (e.key === "Escape") {
                        e.preventDefault();
                        setEditPanel("menu");
                      }
                    }}
                  />
                </div>
                <div className="settings-actions">
                  <button
                    type="button"
                    className="btn"
                    disabled={busy}
                    onClick={() => setEditPanel("menu")}
                  >
                    Back
                  </button>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={busy || !nameDraft.trim()}
                    onClick={() => void commitName()}
                  >
                    {busy ? "Saving…" : "Save name"}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
