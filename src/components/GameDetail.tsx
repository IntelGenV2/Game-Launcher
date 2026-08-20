import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  CoverChoiceGroup,
  Game,
  GameStats,
  STORE_LABELS,
  Store,
  formatLastPlayed,
  formatPlaytime,
  localCoverUrl,
} from "../types";
import { BarChart } from "./Charts";
import { CoverImg } from "./CoverImg";
import { CoverPickModal } from "./CoverPickModal";

interface Props {
  game: Game;
  coverDataUrl?: string | null;
  coverHidden?: boolean;
  onBack: () => void;
  onLaunch: (game: Game) => void;
  onOpenSaveFolder: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onRemove: (game: Game) => void;
  onGameUpdated: (game: Game) => void;
  onCoverUpdated?: (id: string, dataUrl: string) => void;
}

type Tab = "overview" | "sessions" | "notes";
type EditPanel = "menu" | "name" | "launch" | null;

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

const TABS: { id: Tab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "sessions", label: "Sessions" },
  { id: "notes", label: "Notes" },
];

export function GameDetail({
  game,
  coverDataUrl,
  coverHidden = false,
  onBack,
  onLaunch,
  onOpenSaveFolder,
  onToggleFavorite,
  onRemove,
  onGameUpdated,
  onCoverUpdated,
}: Props) {
  const [stats, setStats] = useState<GameStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [editPanel, setEditPanel] = useState<EditPanel>(null);
  const [nameDraft, setNameDraft] = useState(game.name);
  const [notesDraft, setNotesDraft] = useState(game.notes ?? "");
  const [tagDraft, setTagDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<Tab>("overview");
  const [metaBusy, setMetaBusy] = useState(false);
  const [argsDraft, setArgsDraft] = useState(game.launchArgs ?? "");
  const [cwdDraft, setCwdDraft] = useState(game.workingDir ?? "");
  const [adminDraft, setAdminDraft] = useState(game.runAsAdmin);
  const [saveDraft, setSaveDraft] = useState(game.saveFolder ?? "");
  const [coverPick, setCoverPick] = useState<CoverChoiceGroup | null>(null);
  const [full, setFull] = useState<Game>(game);
  const pageRef = useRef<HTMLDivElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);
  const view =
    full.id === game.id
      ? {
          ...full,
          ...game,
          description: full.description ?? game.description,
          notes: full.notes ?? game.notes,
          launchArgs: full.launchArgs ?? game.launchArgs,
          workingDir: full.workingDir ?? game.workingDir,
          saveFolder: full.saveFolder ?? game.saveFolder,
        }
      : game;
  const pathLabel = pathLabelFor(view);
  const genres = view.genres?.length
    ? view.genres
    : view.genre
      ? view.genre.split(",").map((s) => s.trim())
      : [];

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
    let cancelled = false;
    setLoading(true);
    setEditPanel(null);
    setTab("overview");
    setFull(game);
    setNameDraft(game.name);
    setNotesDraft(game.notes ?? "");
    setArgsDraft(game.launchArgs ?? "");
    setCwdDraft(game.workingDir ?? "");
    setAdminDraft(game.runAsAdmin);
    setSaveDraft(game.saveFolder ?? "");
    refreshStats();
    scrollDetailToTop();
    requestAnimationFrame(() => {
      scrollDetailToTop();
      requestAnimationFrame(scrollDetailToTop);
    });

    invoke<Game>("get_game", { id: game.id })
      .then((loaded) => {
        if (cancelled) return;
        setFull(loaded);
        setNotesDraft(loaded.notes ?? "");
        setArgsDraft(loaded.launchArgs ?? "");
        setCwdDraft(loaded.workingDir ?? "");
        setAdminDraft(loaded.runAsAdmin);
        setSaveDraft(loaded.saveFolder ?? "");
        const needsMeta =
          !loaded.developer ||
          !loaded.description ||
          !loaded.releaseYear ||
          !(loaded.genres && loaded.genres.length);
        if (!needsMeta) return;
        setMetaBusy(true);
        return invoke<Game>("fetch_game_metadata", { id: game.id })
          .then((updated) => {
            if (cancelled) return;
            setFull(updated);
            onGameUpdated(updated);
          })
          .finally(() => {
            if (!cancelled) setMetaBusy(false);
          });
      })
      .catch(() => undefined);

    if (!game.coverPath) {
      invoke("fetch_covers", { ids: [game.id] }).catch(() => undefined);
    }
    return () => {
      cancelled = true;
    };
  }, [game.id]);

  useEffect(() => {
    setNameDraft(game.name);
  }, [game.name]);

  useEffect(() => {
    if (editPanel === "name") {
      nameInputRef.current?.focus();
      nameInputRef.current?.select();
    }
  }, [editPanel]);

  async function applyCoverUpdate(updated: Game) {
    setFull(updated);
    onGameUpdated(updated);
    if (updated.coverPath) {
      const url = localCoverUrl(updated.coverPath, true);
      if (url) onCoverUpdated?.(game.id, url);
    }
    setCoverPick(null);
    setEditPanel(null);
  }

  async function browseCoverFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_custom_cover", { id: game.id, path: selected });
      await applyCoverUpdate(updated);
    } finally {
      setBusy(false);
    }
  }

  async function handleSetCover() {
    setBusy(true);
    try {
      const group = await invoke<CoverChoiceGroup>("list_cover_choices", { id: game.id });
      if (group.paths.length >= 2) {
        setCoverPick(group);
        return;
      }
    } catch {
      /* fall through to file picker */
    } finally {
      setBusy(false);
    }
    await browseCoverFile();
  }

  async function handleChooseCover(path: string) {
    setBusy(true);
    try {
      const updated = await invoke<Game>("choose_cover", { id: game.id, path });
      await applyCoverUpdate(updated);
    } finally {
      setBusy(false);
    }
  }

  async function handleSetPath() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_game_path", { id: game.id, path: selected });
      onGameUpdated(updated);
      setFull(updated);
      setEditPanel(null);
    } finally {
      setBusy(false);
    }
  }

  async function commitName() {
    const name = nameDraft.trim();
    if (!name) return;
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_game_name", { id: game.id, name });
      onGameUpdated(updated);
      setFull(updated);
      setEditPanel("menu");
    } finally {
      setBusy(false);
    }
  }

  async function saveNotes() {
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_notes", { id: game.id, notes: notesDraft });
      onGameUpdated(updated);
      setFull(updated);
    } finally {
      setBusy(false);
    }
  }

  async function saveTags(next: string[]) {
    const updated = await invoke<Game>("set_tags", { id: game.id, tags: next });
    onGameUpdated(updated);
    setFull(updated);
  }

  async function saveLaunchOptions() {
    setBusy(true);
    try {
      const updated = await invoke<Game>("set_launch_options", {
        id: game.id,
        launchArgs: argsDraft.trim() || null,
        workingDir: cwdDraft.trim() || null,
        runAsAdmin: adminDraft,
        saveFolder: saveDraft.trim() || null,
      });
      onGameUpdated(updated);
      setFull(updated);
      setEditPanel(null);
    } finally {
      setBusy(false);
    }
  }

  async function pickPath(kind: "cwd" | "save") {
    const selected = await open({
      multiple: false,
      directory: true,
    });
    if (!selected || typeof selected !== "string") return;
    if (kind === "cwd") setCwdDraft(selected);
    if (kind === "save") setSaveDraft(selected);
  }

  const dailyPoints =
    stats?.dailyPlaytime.map((d) => ({
      label: d.day.slice(5),
      value: d.minutes,
    })) ?? [];

  const editModal =
    editPanel &&
    createPortal(
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
                <button type="button" className="edit-choice" disabled={busy} onClick={() => void handleSetCover()}>
                  <span className="edit-choice-title">Cover art</span>
                  <span className="edit-choice-desc">Choose among saved images, or pick a new file</span>
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
                <button type="button" className="edit-choice" disabled={busy} onClick={() => void handleSetPath()}>
                  <span className="edit-choice-title">Game path</span>
                  <span className="edit-choice-desc">Point at a different .exe (kept after refresh)</span>
                </button>
                <button type="button" className="edit-choice" disabled={busy} onClick={() => setEditPanel("launch")}>
                  <span className="edit-choice-title">Launch options</span>
                  <span className="edit-choice-desc">Args, working folder, admin, save folder</span>
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
                <button type="button" className="btn" disabled={busy} onClick={() => setEditPanel("menu")}>
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

          {editPanel === "launch" && (
            <>
              <h2>Launch options</h2>
              <div className="field">
                <label>Extra arguments</label>
                <input value={argsDraft} onChange={(e) => setArgsDraft(e.target.value)} placeholder="-windowed" />
              </div>
              <div className="field">
                <label>Working directory</label>
                <div className="path-row">
                  <input value={cwdDraft} onChange={(e) => setCwdDraft(e.target.value)} />
                  <button type="button" className="btn" onClick={() => void pickPath("cwd")}>
                    Browse
                  </button>
                </div>
              </div>
              <label className="toggle-row">
                <input type="checkbox" checked={adminDraft} onChange={(e) => setAdminDraft(e.target.checked)} />
                Run as administrator
              </label>
              <div className="field">
                <label>Save folder</label>
                <div className="path-row">
                  <input value={saveDraft} onChange={(e) => setSaveDraft(e.target.value)} />
                  <button type="button" className="btn" onClick={() => void pickPath("save")}>
                    Browse
                  </button>
                </div>
              </div>
              <div className="settings-actions">
                <button type="button" className="btn" disabled={busy} onClick={() => setEditPanel("menu")}>
                  Back
                </button>
                <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void saveLaunchOptions()}>
                  {busy ? "Saving…" : "Save"}
                </button>
              </div>
            </>
          )}
        </div>
      </div>,
      document.body,
    );

  return (
    <div className="detail-page" ref={pageRef}>
      <button type="button" className="btn back-btn" onClick={onBack}>
        ← Library
      </button>

      <div className="detail-hero">
        <div className={`detail-cover${coverHidden ? " fly-hidden" : ""}`}>
          <CoverImg
            game={game}
            override={coverDataUrl}
            fallbackClassName="cover-fallback large"
            loading="eager"
          />
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
          <h2 className="detail-title">{game.name}</h2>
          <dl className="detail-facts">
            <div>
              <dt>Store</dt>
              <dd>{STORE_LABELS[game.store as Store] ?? game.store}</dd>
            </div>
            <div>
              <dt>Year</dt>
              <dd>{view.releaseYear ?? "—"}</dd>
            </div>
            <div>
              <dt>Game time</dt>
              <dd>{formatPlaytime(game.playtimeMinutes)}</dd>
            </div>
            <div>
              <dt>Last played</dt>
              <dd>{formatLastPlayed(game.lastPlayedAt)}</dd>
            </div>
            {(view.developer || view.publisher) && (
              <div className="span-2">
                <dt>Made by</dt>
                <dd>{[view.developer, view.publisher].filter(Boolean).join(" · ")}</dd>
              </div>
            )}
            {genres.length > 0 && (
              <div className="span-2">
                <dt>Genres</dt>
                <dd>{genres.slice(0, 6).join(", ")}</dd>
              </div>
            )}
            {game.missing && (
              <div>
                <dt>Status</dt>
                <dd className="warn">Missing</dd>
              </div>
            )}
            {metaBusy && (
              <div>
                <dt>Info</dt>
                <dd>Fetching…</dd>
              </div>
            )}
          </dl>
          <p className="detail-path" title={pathLabel}>
            {pathLabel}
          </p>

          <div className="detail-actions">
            <button
              type="button"
              className="btn btn-primary"
              disabled={game.missing}
              onClick={() => onLaunch(game)}
            >
              ▶ Play
            </button>
            {view.saveFolder ? (
              <button type="button" className="btn" onClick={() => onOpenSaveFolder(game)}>
                Save folder
              </button>
            ) : null}
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
              Remove
            </button>
          </div>
        </div>
      </div>

      <div className="detail-tabs" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={`detail-tab${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <div className="detail-tab-body">
          {view.description ? <p className="game-blurb">{view.description}</p> : null}
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
              <span className="stat-label">Year</span>
              <span className="stat-value">{view.releaseYear ?? "—"}</span>
            </div>
          </div>
          <div className="tag-editor">
            {(game.tags ?? []).map((tag) => (
              <button
                type="button"
                key={tag}
                className="tag-chip"
                onClick={() => void saveTags((game.tags ?? []).filter((t) => t !== tag))}
                title="Remove tag"
              >
                {tag} ×
              </button>
            ))}
            <form
              className="tag-add"
              onSubmit={(e) => {
                e.preventDefault();
                const t = tagDraft.trim();
                if (!t) return;
                const next = [...(game.tags ?? [])];
                if (!next.some((x) => x.toLowerCase() === t.toLowerCase())) next.push(t);
                setTagDraft("");
                void saveTags(next);
              }}
            >
              <input
                value={tagDraft}
                onChange={(e) => setTagDraft(e.target.value)}
                placeholder="Add tag…"
                maxLength={32}
              />
            </form>
          </div>
          <div className="charts-row">
            <BarChart
              title="Playtime (last 14 days)"
              points={dailyPoints}
              formatValue={(v) => `${v}m`}
            />
          </div>
        </div>
      )}

      {tab === "sessions" && (
        <div className="sessions-panel">
          <h3>Recent sessions</h3>
          {!stats || stats.sessions.length === 0 ? (
            <p className="chart-empty">No sessions yet. Hit Play to start tracking.</p>
          ) : (
            <ul className="session-list">
              {[...stats.sessions]
                .reverse()
                .slice(0, 24)
                .map((s) => (
                  <li key={s.id}>
                    <span>{new Date(s.startedAt).toLocaleString()}</span>
                    <span>{s.durationMinutes > 0 ? `${s.durationMinutes}m` : "in progress"}</span>
                  </li>
                ))}
            </ul>
          )}
        </div>
      )}

      {tab === "notes" && (
        <div className="notes-panel">
          <h3>Your notes</h3>
          <textarea
            value={notesDraft}
            onChange={(e) => setNotesDraft(e.target.value)}
            rows={10}
            placeholder="Install quirks, save locations, launch flags…"
          />
          <div className="settings-actions">
            <button
              type="button"
              className="btn btn-primary"
              disabled={busy || notesDraft === (view.notes ?? "")}
              onClick={() => void saveNotes()}
            >
              {busy ? "Saving…" : "Save notes"}
            </button>
          </div>
        </div>
      )}

      {editModal}

      {coverPick &&
        createPortal(
          <CoverPickModal
            group={coverPick}
            index={0}
            total={1}
            onPick={(path) => void handleChooseCover(path)}
            onSkip={() => setCoverPick(null)}
            onBrowse={() => {
              setCoverPick(null);
              void browseCoverFile();
            }}
          />,
          document.body,
        )}
    </div>
  );
}
