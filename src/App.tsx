import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { GameDetail } from "./components/GameDetail";
import { GameGrid } from "./components/GameGrid";
import { GroupAddModal } from "./components/GroupAddModal";
import { GroupNameModal } from "./components/GroupNameModal";
import { CustomSelect } from "./components/CustomSelect";
import { SettingsModal } from "./components/SettingsModal";
import { UpdateChecker } from "./components/UpdateChecker";
import {
  AppSettings,
  applyAppearance,
  buildDefaultLibraryOrder,
  FILTER_OPTIONS,
  Game,
  GameGroup,
  gameOrderKey,
  groupOrderKey,
  isSortMode,
  isThemeId,
  LibraryFilter,
  LibraryStats,
  parseLibraryOrder,
  reconcileLibraryOrder,
  SORT_OPTIONS,
  SortMode,
  ThemeId,
} from "./types";
import "./styles/theme.css";
import "./styles/App.css";

function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [query, setQuery] = useState("");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sortBy, setSortBy] = useState<SortMode>("name");
  const [toast, setToast] = useState<{ text: string; error?: boolean } | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<AppSettings>({
    steamGridDbApiKey: null,
    sortBy: "name",
    theme: "emerald",
    cardScale: 1,
    libraryOrder: null,
  });
  const [stats, setStats] = useState<LibraryStats>({ total: 0, favorites: 0, missing: 0 });
  const [dataPath, setDataPath] = useState("");
  const [coverMap, setCoverMap] = useState<Record<string, string>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [groups, setGroups] = useState<GameGroup[]>([]);
  const [expandedGroupId, setExpandedGroupId] = useState<string | null>(null);
  const [libraryOrder, setLibraryOrder] = useState<string[]>([]);
  const [addToGroupTarget, setAddToGroupTarget] = useState<GameGroup | null>(null);
  const [groupNameModal, setGroupNameModal] = useState<
    null | { mode: "create" } | { mode: "rename"; group: GameGroup }
  >(null);
  const coverLoaded = useRef<Set<string>>(new Set());
  const sessionStarts = useRef<Record<string, number>>({});
  const toastTimer = useRef<number | null>(null);

  const showHidden = libraryFilter === "hidden";

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

  const applySettingsAppearance = useCallback((s: AppSettings) => {
    const theme: ThemeId = isThemeId(s.theme) ? s.theme : "emerald";
    const scale = typeof s.cardScale === "number" && s.cardScale > 0 ? s.cardScale : 1;
    applyAppearance(theme, scale);
  }, []);

  const refreshGroups = useCallback(async () => {
    try {
      const list = await invoke<GameGroup[]>("list_groups");
      setGroups(list);
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
      if (isSortMode(s.sortBy)) setSortBy(s.sortBy);
      applySettingsAppearance(s);
      setDataPath(path);

      let list = await invoke<Game[]>("list_games");
      setGames(list);
      const groupList = await invoke<GameGroup[]>("list_groups");
      setGroups(groupList);
      const savedOrder = parseLibraryOrder(s.libraryOrder);
      setLibraryOrder(
        reconcileLibraryOrder(
          savedOrder.length ? savedOrder : buildDefaultLibraryOrder(list, groupList),
          list,
          groupList,
        ),
      );

      if (list.length === 0) {
        setScanning(true);
        list = await invoke<Game[]>("rescan_library");
        setGames(list);
        setScanning(false);
      }
      await refreshStats();

      invoke("fetch_covers", { ids: null }).catch(() => undefined);
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setLoading(false);
      setScanning(false);
    }
  }, [applySettingsAppearance, refreshGroups, refreshStats, showToast]);

  useEffect(() => {
    bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    if (!selectedId) return;
    const main = document.querySelector(".main");
    if (main) main.scrollTop = 0;
    requestAnimationFrame(() => {
      if (main) main.scrollTop = 0;
    });
  }, [selectedId]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<{
        id: string;
        coverPath: string;
        steamAppId: string | null;
        coverUrl: string | null;
        genre: string | null;
      }>("cover-updated", (event) => {
        const { id, coverPath, steamAppId, coverUrl, genre } = event.payload;
        setGames((prev) =>
          prev.map((g) =>
            g.id === id
              ? {
                  ...g,
                  coverPath,
                  steamAppId: steamAppId ?? g.steamAppId,
                  coverUrl: coverUrl ?? g.coverUrl,
                  genre: genre ?? g.genre,
                }
              : g,
          ),
        );
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

  const selectedGame = useMemo(
    () => games.find((g) => g.id === selectedId) ?? null,
    [games, selectedId],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    let list = games.filter((g) => {
      if (libraryFilter === "hidden") return g.hidden;
      if (g.hidden) return false;
      if (libraryFilter === "favorites" && !g.favorite) return false;
      if (libraryFilter === "other") {
        if (g.store !== "manual" && g.store !== "roblox") return false;
      } else if (
        libraryFilter !== "all" &&
        libraryFilter !== "favorites" &&
        g.store !== libraryFilter
      ) {
        return false;
      }
      if (q && !g.name.toLowerCase().includes(q)) return false;
      return true;
    });

    list = [...list].sort((a, b) => {
      if (sortBy === "custom") {
        const ai = libraryOrder.indexOf(gameOrderKey(a.id));
        const bi = libraryOrder.indexOf(gameOrderKey(b.id));
        const av = ai < 0 ? Number.MAX_SAFE_INTEGER : ai;
        const bv = bi < 0 ? Number.MAX_SAFE_INTEGER : bi;
        return av - bv || a.name.localeCompare(b.name);
      }
      if (sortBy === "favorites") {
        if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
        return a.name.localeCompare(b.name);
      }
      if (sortBy === "missing") {
        if (a.missing !== b.missing) return a.missing ? -1 : 1;
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
      if (sortBy === "added") {
        return Date.parse(b.dateAdded) - Date.parse(a.dateAdded) || a.name.localeCompare(b.name);
      }
      if (sortBy === "nameDesc") {
        return b.name.localeCompare(a.name);
      }
      return a.name.localeCompare(b.name);
    });
    return list;
  }, [games, query, libraryFilter, sortBy, libraryOrder]);

  // Keep library order in sync when games/groups change
  useEffect(() => {
    setLibraryOrder((prev) => reconcileLibraryOrder(prev, games, groups));
  }, [games, groups]);

  const persistLibraryOrder = useCallback(
    (nextOrder: string[], forceCustom = true) => {
      const reconciled = reconcileLibraryOrder(nextOrder, games, groups);
      setLibraryOrder(reconciled);
      const nextSort: SortMode = forceCustom ? "custom" : sortBy;
      if (forceCustom) setSortBy("custom");
      const nextSettings: AppSettings = {
        ...settings,
        sortBy: nextSort,
        libraryOrder: JSON.stringify(reconciled),
      };
      setSettings(nextSettings);
      invoke("save_settings", { settings: nextSettings }).catch(() => undefined);
    },
    [games, groups, settings, sortBy],
  );

  const libraryOrderRef = useRef(libraryOrder);
  const groupsRef = useRef(groups);
  const persistOrderRef = useRef(persistLibraryOrder);
  libraryOrderRef.current = libraryOrder;
  groupsRef.current = groups;
  persistOrderRef.current = persistLibraryOrder;

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

  const addManualFromPath = useCallback(
    async (path: string, groupId?: string | null) => {
      const game = await invoke<Game>("add_manual_game", { path });
      setGames((prev) => {
        const rest = prev.filter((g) => g.id !== game.id);
        return [...rest, game].sort((a, b) => a.name.localeCompare(b.name));
      });

      const order = libraryOrderRef.current.filter((k) => k !== gameOrderKey(game.id));
      const group = groupId ? groupsRef.current.find((g) => g.id === groupId) : null;

      if (group) {
        await invoke<GameGroup>("add_game_to_group", {
          groupId: group.id,
          gameId: game.id,
        });
        await refreshGroups();
        persistOrderRef.current(order, true);
        showToast(`Added ${game.name} to “${group.name}”`);
      } else {
        persistOrderRef.current([...order, gameOrderKey(game.id)], true);
        showToast(`Added ${game.name} (saved permanently)`);
      }

      await refreshStats();
      setSelectedId(game.id);
      invoke("fetch_covers", { ids: [game.id] }).catch(() => undefined);
      return game;
    },
    [refreshGroups, refreshStats, showToast],
  );

  async function handleAddManual() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Executable", extensions: ["exe"] }],
      });
      if (!selected || typeof selected !== "string") return;
      await addManualFromPath(selected);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      try {
        const webview = getCurrentWebview();
        const win = getCurrentWindow();
        unlisten = await webview.onDragDropEvent(async (event) => {
          if (event.payload.type !== "drop") return;
          const paths = event.payload.paths.filter((p) =>
            p.toLowerCase().endsWith(".exe"),
          );
          if (paths.length === 0) {
            showToast("Drop a .exe file to add a game", true);
            return;
          }

          let groupId: string | null = null;
          try {
            const factor = await win.scaleFactor();
            const { x, y } = event.payload.position;
            const el = document.elementFromPoint(x / factor, y / factor) as HTMLElement | null;
            groupId = el?.closest<HTMLElement>("[data-drop-group]")?.dataset.dropGroup ?? null;
          } catch {
            /* ignore hit-test failures */
          }

          for (const path of paths) {
            try {
              await addManualFromPath(path, groupId);
            } catch (e) {
              showToast(String(e), true);
            }
          }
        });
        if (cancelled) unlisten();
      } catch {
        /* not running inside Tauri */
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [addManualFromPath, showToast]);

  async function handleLaunch(game: Game) {
    if (game.missing) {
      showToast("This game looks missing from disk", true);
      return;
    }
    try {
      const prev = sessionStarts.current[game.id];
      if (prev) {
        const minutes = Math.max(1, Math.round((Date.now() - prev) / 60000));
        await invoke("end_play_session", { id: game.id, minutes });
      }
      sessionStarts.current[game.id] = Date.now();
      const updated = await invoke<Game>("launch_game", { id: game.id });
      setGames((prev) =>
        prev.map((g) => (g.id === updated.id ? { ...updated, playtimeMinutes: g.playtimeMinutes } : g)),
      );
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

  async function refreshGroupsLocal(next?: GameGroup) {
    if (next) {
      setGroups((prev) => {
        const rest = prev.filter((g) => g.id !== next.id);
        return [...rest, next].sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
      });
      return;
    }
    await refreshGroups();
  }

  async function handleCreateGroup() {
    setGroupNameModal({ mode: "create" });
  }

  async function confirmCreateGroup(name: string) {
    const group = await invoke<GameGroup>("create_group", {
      name,
      gameIds: [],
    });
    await refreshGroupsLocal(group);
    persistLibraryOrder([...libraryOrder, groupOrderKey(group.id)], true);
    showToast(`Created “${group.name}”`);
    setExpandedGroupId(group.id);
  }

  async function handleAddToGroup(game: Game, group: GameGroup) {
    try {
      await invoke<GameGroup>("add_game_to_group", {
        groupId: group.id,
        gameId: game.id,
      });
      await refreshGroups();
      persistLibraryOrder(
        libraryOrder.filter((k) => k !== gameOrderKey(game.id)),
        true,
      );
      showToast(`Added to “${group.name}”`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  function handleRenameGroup(group: GameGroup) {
    setGroupNameModal({ mode: "rename", group });
  }

  async function confirmRenameGroup(group: GameGroup, name: string) {
    const updated = await invoke<GameGroup>("rename_group", {
      id: group.id,
      name,
    });
    await refreshGroupsLocal(updated);
    showToast("Group renamed");
  }

  async function handleDeleteGroup(group: GameGroup) {
    if (!confirm(`Delete group “${group.name}”? Games stay in your library.`)) return;
    try {
      const memberKeys = group.gameIds.map((id) => gameOrderKey(id));
      await invoke("delete_group", { id: group.id });
      setGroups((prev) => prev.filter((g) => g.id !== group.id));
      if (expandedGroupId === group.id) setExpandedGroupId(null);
      const without = libraryOrder.filter((k) => k !== groupOrderKey(group.id));
      const insertAt = Math.max(0, libraryOrder.indexOf(groupOrderKey(group.id)));
      const next = [...without];
      next.splice(insertAt, 0, ...memberKeys.filter((k) => !without.includes(k)));
      persistLibraryOrder(next, true);
      showToast(`Deleted “${group.name}”`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleRemoveFromGroup(group: GameGroup, game: Game) {
    try {
      const updated = await invoke<GameGroup>("remove_game_from_group", {
        groupId: group.id,
        gameId: game.id,
      });
      await refreshGroupsLocal(updated);
      const gKey = groupOrderKey(group.id);
      const next = libraryOrder.filter((k) => k !== gameOrderKey(game.id));
      const idx = next.indexOf(gKey);
      if (idx >= 0) next.splice(idx + 1, 0, gameOrderKey(game.id));
      else next.push(gameOrderKey(game.id));
      persistLibraryOrder(next, true);
      showToast(`Removed from “${group.name}”`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  function persistSort(v: SortMode) {
    setSortBy(v);
    const next = { ...settings, sortBy: v, libraryOrder: JSON.stringify(libraryOrder) };
    setSettings(next);
    invoke("save_settings", { settings: next }).catch(() => undefined);
  }

  // Save order when leaving the app
  useEffect(() => {
    const save = () => {
      const payload: AppSettings = {
        ...settings,
        sortBy,
        libraryOrder: JSON.stringify(libraryOrder),
      };
      invoke("save_settings", { settings: payload }).catch(() => undefined);
    };
    window.addEventListener("beforeunload", save);
    return () => window.removeEventListener("beforeunload", save);
  }, [settings, sortBy, libraryOrder]);

  return (
    <div className="app-shell">
      <header className="topbar">
        <div className="brand">
          <img src="/intelgen-icon.png" alt="IntelGen" className="brand-mark" />
          <h1 className="brand-name">IntelGen</h1>
        </div>
        {!selectedGame && (
          <div className="topbar-tools">
            <div className="search-wrap">
              <span className="icon">⌕</span>
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search…"
                aria-label="Search games"
              />
            </div>
            <CustomSelect
              value={libraryFilter}
              options={FILTER_OPTIONS}
              onChange={setLibraryFilter}
              ariaLabel="Filter library"
            />
            <CustomSelect
              value={sortBy}
              options={SORT_OPTIONS}
              onChange={persistSort}
              ariaLabel="Sort games"
            />
            <button type="button" className="btn" onClick={() => void handleCreateGroup()}>
              Create group
            </button>
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

      <main className="main">
        {selectedGame ? (
          <GameDetail
            game={selectedGame}
            coverDataUrl={coverMap[selectedGame.id]}
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
                groups={groups}
                coverMap={coverMap}
                libraryOrder={
                  sortBy === "custom"
                    ? libraryOrder
                    : buildDefaultLibraryOrder(filtered, groups)
                }
                expandedGroupId={expandedGroupId}
                onExpandedGroupChange={setExpandedGroupId}
                onOpen={(g) => {
                  setSelectedId(g.id);
                }}
                onLaunch={handleLaunch}
                onToggleFavorite={handleToggleFavorite}
                onHide={handleHide}
                onOpenFolder={handleOpenFolder}
                onAddGame={handleAddManual}
                onRenameGroup={handleRenameGroup}
                onDeleteGroup={handleDeleteGroup}
                onAddToGroup={handleAddToGroup}
                onRemoveFromGroup={handleRemoveFromGroup}
                onReorder={(next) => persistLibraryOrder(next, true)}
                onAddGamesToGroup={(g) => setAddToGroupTarget(g)}
              />
            )}
          </>
        )}
      </main>

      <GroupAddModal
        open={addToGroupTarget != null}
        group={addToGroupTarget}
        members={
          addToGroupTarget
            ? addToGroupTarget.gameIds
                .map((id) => games.find((g) => g.id === id))
                .filter((g): g is Game => Boolean(g))
            : []
        }
        allGames={games}
        coverMap={coverMap}
        onClose={() => setAddToGroupTarget(null)}
        onAdd={(game) => {
          if (addToGroupTarget) void handleAddToGroup(game, addToGroupTarget);
        }}
      />

      <GroupNameModal
        open={groupNameModal != null}
        title={groupNameModal?.mode === "rename" ? "Rename group" : "Create group"}
        initialName={
          groupNameModal?.mode === "rename" ? groupNameModal.group.name : "New group"
        }
        confirmLabel={groupNameModal?.mode === "rename" ? "Rename" : "Create"}
        onClose={() => setGroupNameModal(null)}
        onConfirm={async (name) => {
          try {
            if (groupNameModal?.mode === "rename") {
              await confirmRenameGroup(groupNameModal.group, name);
            } else {
              await confirmCreateGroup(name);
            }
          } catch (e) {
            showToast(String(e), true);
            throw e;
          }
        }}
      />

      <SettingsModal
        open={settingsOpen}
        settings={settings}
        dataPath={dataPath}
        onClose={() => setSettingsOpen(false)}
        onSave={async (next) => {
          await invoke("save_settings", { settings: next });
          setSettings(next);
          if (isSortMode(next.sortBy)) setSortBy(next.sortBy);
          applySettingsAppearance(next);
          showToast("Settings saved");
          invoke("fetch_covers", { ids: null }).catch(() => undefined);
        }}
        onPreviewAppearance={(theme, cardScale) => {
          applyAppearance(theme, cardScale);
        }}
      />

      <UpdateChecker />

      {toast && (
        <div className={`toast${toast.error ? " error" : ""}`} role="status">
          {toast.text}
        </div>
      )}
    </div>
  );
}

export default App;
