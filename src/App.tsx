import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Filters } from "./components/Filters";
import { GameDetail } from "./components/GameDetail";
import { GameGrid } from "./components/GameGrid";
import { SettingsModal } from "./components/SettingsModal";
import {
  AppSettings,
  Game,
  LibraryStats,
  SortMode,
  Store,
} from "./types";
import "./styles/theme.css";
import "./styles/App.css";

function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [query, setQuery] = useState("");
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [showHidden, setShowHidden] = useState(false);
  const [activeStores, setActiveStores] = useState<Set<Store>>(new Set());
  const [liveFps, setLiveFps] = useState<{ gameId: string; fps: number } | null>(null);
  const [sortBy, setSortBy] = useState<SortMode>("name");
  const [toast, setToast] = useState<{ text: string; error?: boolean } | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({
    steamGridDbApiKey: null,
    sortBy: "name",
  });
  const [stats, setStats] = useState<LibraryStats>({ total: 0, favorites: 0, missing: 0 });
  const [dataPath, setDataPath] = useState("");
  const [coverMap, setCoverMap] = useState<Record<string, string>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const coverLoaded = useRef<Set<string>>(new Set());
  const sessionStarts = useRef<Record<string, number>>({});
  const toastTimer = useRef<number | null>(null);

  const showToast = useCallback((text: string, error = false) => {
    setToast({ text, error });
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3200);
  }, []);

  const refreshStats = useCallback(async () => {
    try {
      const s = await invoke<LibraryStats>("library_stats");
      setStats(s);
    } catch {
      /* ignore */
    }
  }, []);

  const bootstrap = useCallback(async () => {
    setLoading(true);
    try {
      const [s, path] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<string>("app_data_path"),
      ]);
      setSettings(s);
      if (s.sortBy === "recent" || s.sortBy === "playtime" || s.sortBy === "favorites" || s.sortBy === "name") {
        setSortBy(s.sortBy);
      }
      setDataPath(path);

      // Always load persisted library first (manual games survive restarts here)
      let list = await invoke<Game[]>("list_games");
      setGames(list);

      // Rescan in background / on empty
      if (list.length === 0) {
        setScanning(true);
        list = await invoke<Game[]>("rescan_library");
        setGames(list);
        setScanning(false);
      }
      await refreshStats();

      // Background cover fetch — never blocks the UI thread
      invoke("fetch_covers", { ids: null }).catch(() => undefined);
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setLoading(false);
      setScanning(false);
    }
  }, [refreshStats, showToast]);

  useEffect(() => {
    bootstrap();
  }, [bootstrap]);

  // Keep stats page pinned to the top of the scroll pane
  useEffect(() => {
    if (!selectedId) return;
    const main = document.querySelector(".main");
    if (main) main.scrollTop = 0;
    requestAnimationFrame(() => {
      if (main) main.scrollTop = 0;
    });
  }, [selectedId]);

  // Apply covers as they finish downloading (non-blocking)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<{
        id: string;
        coverPath: string;
        steamAppId: string | null;
        coverUrl: string | null;
      }>("cover-updated", (event) => {
        const { id, coverPath, steamAppId, coverUrl } = event.payload;
        setGames((prev) =>
          prev.map((g) =>
            g.id === id
              ? {
                  ...g,
                  coverPath,
                  steamAppId: steamAppId ?? g.steamAppId,
                  coverUrl: coverUrl ?? g.coverUrl,
                }
              : g,
          ),
        );
        // Immediately hydrate this tile — don't wait for a full grid reload
        invoke<string | null>("get_cover_data_url", { id })
          .then((url) => {
            if (url) {
              coverLoaded.current.add(id);
              setCoverMap((m) => ({ ...m, [id]: url }));
            }
          })
          .catch(() => undefined);
      });
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // One-shot batch load of all local covers for the home grid.
  // Important: do NOT cancel/re-mark ids when `games` updates (that was dropping art).
  useEffect(() => {
    const needing = games
      .filter((g) => g.coverPath && !coverLoaded.current.has(g.id) && !coverMap[g.id])
      .map((g) => g.id);
    if (needing.length === 0) return;

    let alive = true;
    (async () => {
      try {
        const map = await invoke<Record<string, string>>("get_cover_data_urls", {
          ids: needing,
        });
        if (!alive) return;
        for (const id of Object.keys(map)) {
          coverLoaded.current.add(id);
        }
        setCoverMap((prev) => ({ ...prev, ...map }));
      } catch {
        // Fallback: load individually so one failure can't blank the grid
        if (!alive) return;
        for (const id of needing) {
          if (!alive) return;
          try {
            const url = await invoke<string | null>("get_cover_data_url", { id });
            if (url) {
              coverLoaded.current.add(id);
              setCoverMap((m) => ({ ...m, [id]: url }));
            }
          } catch {
            /* ignore */
          }
        }
      }
    })();

    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [games]);

  const hiddenBoot = useRef(true);
  useEffect(() => {
    if (hiddenBoot.current) {
      hiddenBoot.current = false;
      if (!showHidden) return;
    }
    let cancelled = false;
    (async () => {
      try {
        if (showHidden) {
          const hidden = await invoke<Game[]>("list_hidden_games");
          if (!cancelled) setGames(hidden);
        } else {
          const list = await invoke<Game[]>("list_games");
          if (!cancelled) setGames(list);
        }
      } catch (e) {
        showToast(String(e), true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [showHidden, showToast]);

  // Live FPS from PresentMon
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let lastLogged = 0;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<{ gameId: string; fps: number; processAlive: boolean }>(
        "fps-tick",
        (event) => {
          const { gameId, fps, processAlive } = event.payload;
          if (!processAlive) {
            setLiveFps((prev) => (prev?.gameId === gameId ? null : prev));
            return;
          }
          if (fps > 0) {
            setLiveFps({ gameId, fps });
            const now = Date.now();
            if (now - lastLogged > 15000) {
              lastLogged = now;
              invoke("record_live_fps", { id: gameId, fps }).catch(() => undefined);
            }
          }
        },
      );
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  const selectedGame = useMemo(
    () => games.find((g) => g.id === selectedId) ?? null,
    [games, selectedId],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    let list = games.filter((g) => {
      if (showHidden) return g.hidden;
      if (g.hidden) return false;
      if (favoritesOnly && !g.favorite) return false;
      if (activeStores.size > 0 && !activeStores.has(g.store)) return false;
      if (q && !g.name.toLowerCase().includes(q)) return false;
      return true;
    });

    list = [...list].sort((a, b) => {
      if (sortBy === "favorites") {
        if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
        return a.name.localeCompare(b.name);
      }
      if (sortBy === "playtime") {
        return b.playtimeMinutes - a.playtimeMinutes || a.name.localeCompare(b.name);
      }
      if (sortBy === "recent") {
        const at = a.lastPlayedAt ? Date.parse(a.lastPlayedAt) : 0;
        const bt = b.lastPlayedAt ? Date.parse(b.lastPlayedAt) : 0;
        return bt - at || a.name.localeCompare(b.name);
      }
      return a.name.localeCompare(b.name);
    });
    return list;
  }, [games, query, favoritesOnly, activeStores, sortBy, showHidden]);

  async function handleRescan() {
    setScanning(true);
    try {
      const list = await invoke<Game[]>("rescan_library");
      setGames(list);
      await refreshStats();
      showToast(`Library updated · ${list.length} games`);
      invoke("fetch_covers", { ids: null }).catch(() => undefined);
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setScanning(false);
    }
  }

  async function handleAddManual() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Executable", extensions: ["exe"] }],
      });
      if (!selected || typeof selected !== "string") return;
      const game = await invoke<Game>("add_manual_game", { path: selected });
      setGames((prev) => {
        const rest = prev.filter((g) => g.id !== game.id);
        return [...rest, game].sort((a, b) => a.name.localeCompare(b.name));
      });
      await refreshStats();
      showToast(`Added ${game.name} (saved permanently)`);
      setSelectedId(game.id);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleLaunch(game: Game) {
    if (game.missing) {
      showToast("This game looks missing from disk", true);
      return;
    }
    try {
      const prev = sessionStarts.current[game.id];
      if (prev) {
        const minutes = Math.max(1, Math.round((Date.now() - prev) / 60000));
        await invoke("end_play_session", { id: game.id, minutes, avgFps: null });
      }
      sessionStarts.current[game.id] = Date.now();
      const updated = await invoke<Game>("launch_game", { id: game.id });
      setGames((prev) => prev.map((g) => (g.id === updated.id ? { ...updated, playtimeMinutes: g.playtimeMinutes } : g)));
      showToast(`Launching ${game.name}`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  useEffect(() => {
    const onFocus = async () => {
      for (const [id, started] of Object.entries(sessionStarts.current)) {
        const minutes = Math.round((Date.now() - started) / 60000);
        if (minutes >= 1) {
          try {
            const updated = await invoke<Game>("end_play_session", {
              id,
              minutes,
              avgFps: null,
            });
            setGames((prev) => prev.map((g) => (g.id === updated.id ? updated : g)));
            delete sessionStarts.current[id];
          } catch {
            /* ignore */
          }
        }
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  async function handleToggleFavorite(game: Game) {
    try {
      const updated = await invoke<Game>("toggle_favorite", { id: game.id });
      setGames((prev) => prev.map((g) => (g.id === updated.id ? updated : g)));
      await refreshStats();
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleHide(game: Game) {
    try {
      const hide = !game.hidden;
      const updated = await invoke<Game>("set_hidden", { id: game.id, hidden: hide });
      if (showHidden) {
        if (!hide) {
          // unhidden — remove from hidden view
          setGames((prev) => prev.filter((g) => g.id !== game.id));
          if (selectedId === game.id) setSelectedId(null);
        } else {
          setGames((prev) => prev.map((g) => (g.id === updated.id ? updated : g)));
        }
      } else if (hide) {
        setGames((prev) => prev.filter((g) => g.id !== game.id));
        if (selectedId === game.id) setSelectedId(null);
      }
      await refreshStats();
      showToast(hide ? `Hidden ${game.name}` : `Restored ${game.name}`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleRemove(game: Game) {
    try {
      await invoke("remove_game", { id: game.id });
      setGames((prev) => prev.filter((g) => g.id !== game.id));
      setCoverMap((m) => {
        const next = { ...m };
        delete next[game.id];
        return next;
      });
      coverLoaded.current.delete(game.id);
      if (selectedId === game.id) setSelectedId(null);
      await refreshStats();
      showToast(`Removed ${game.name}`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleOpenFolder(game: Game) {
    try {
      await invoke("open_install_folder", { id: game.id });
    } catch (e) {
      showToast(String(e), true);
    }
  }

  function toggleStore(store: Store) {
    setActiveStores((prev) => {
      const next = new Set(prev);
      if (next.has(store)) next.delete(store);
      else next.add(store);
      return next;
    });
  }

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <img src="/intelgen-icon.png" alt="" className="brand-mark" />
          <span className="brand-name">IntelGen</span>
        </div>
        {!selectedGame && (
          <div className="search-wrap">
            <span className="icon">⌕</span>
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search games…"
              aria-label="Search games"
            />
          </div>
        )}
        <div className="top-actions">
          <button type="button" className="btn" onClick={handleAddManual}>
            Add game
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleRescan}
            disabled={scanning}
          >
            {scanning ? "Scanning…" : "Rescan"}
          </button>
          <button type="button" className="btn" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
        </div>
      </header>

      {!selectedGame && (
        <Filters
          activeStores={activeStores}
          favoritesOnly={favoritesOnly}
          showHidden={showHidden}
          onToggleStore={toggleStore}
          onToggleFavorites={() => {
            setFavoritesOnly((v) => !v);
            setShowHidden(false);
          }}
          onToggleHidden={() => {
            setShowHidden((v) => !v);
            setFavoritesOnly(false);
            setActiveStores(new Set());
          }}
          onClearStores={() => {
            setActiveStores(new Set());
            setShowHidden(false);
          }}
        />
      )}

      <main className="main">
        {selectedGame ? (
          <GameDetail
            game={selectedGame}
            coverDataUrl={coverMap[selectedGame.id]}
            liveFps={liveFps?.gameId === selectedGame.id ? liveFps.fps : null}
            onBack={() => setSelectedId(null)}
            onLaunch={handleLaunch}
            onToggleFavorite={handleToggleFavorite}
            onRemove={handleRemove}
            onGameUpdated={(g) => setGames((prev) => prev.map((x) => (x.id === g.id ? g : x)))}
            onCoverUpdated={(id, dataUrl) => {
              coverLoaded.current.add(id);
              setCoverMap((m) => ({ ...m, [id]: dataUrl }));
            }}
          />
        ) : (
          <>
            <div className="status-bar">
              <div>
                {loading || scanning ? (
                  <span className="loading-pulse">
                    <span className="dot" />
                    {scanning ? "Scanning libraries…" : "Loading library…"}
                  </span>
                ) : (
                  <span>
                    Showing {filtered.length} of {stats.total}
                    {stats.favorites > 0 ? ` · ${stats.favorites} favorites` : ""}
                    {stats.missing > 0 ? ` · ${stats.missing} missing` : ""}
                  </span>
                )}
              </div>
              <select
                className="sort-select"
                value={sortBy}
                onChange={(e) => {
                  const v = e.target.value as SortMode;
                  setSortBy(v);
                  const next = { ...settings, sortBy: v };
                  setSettings(next);
                  invoke("save_settings", { settings: next }).catch(() => undefined);
                }}
                aria-label="Sort games"
              >
                <option value="name">A–Z</option>
                <option value="recent">Recently played</option>
                <option value="playtime">Playtime</option>
                <option value="favorites">Favorites first</option>
              </select>
            </div>

            {!loading && games.length === 0 ? (
              <div className="empty">
                <h2>No games found</h2>
                <p>Install a store library or add a game manually, then hit Rescan.</p>
                <button type="button" className="btn btn-primary" onClick={handleRescan}>
                  Rescan library
                </button>
              </div>
            ) : (
              <GameGrid
                games={filtered}
                coverMap={coverMap}
                onOpen={(g) => {
                  setSelectedId(g.id);
                }}
                onLaunch={handleLaunch}
                onToggleFavorite={handleToggleFavorite}
                onHide={handleHide}
                onOpenFolder={handleOpenFolder}
              />
            )}
          </>
        )}
      </main>

      <SettingsModal
        open={settingsOpen}
        settings={settings}
        dataPath={dataPath}
        onClose={() => setSettingsOpen(false)}
        onSave={async (next) => {
          await invoke("save_settings", { settings: next });
          setSettings(next);
          showToast("Settings saved");
          invoke("fetch_covers", { ids: null }).catch(() => undefined);
        }}
      />

      {toast && (
        <div className={`toast${toast.error ? " error" : ""}`} role="status">
          {toast.text}
        </div>
      )}
    </div>
  );
}

export default App;
