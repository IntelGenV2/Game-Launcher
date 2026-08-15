import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AddToGroupModal } from "./components/AddToGroupModal";
import { BulkAddToGroupModal } from "./components/BulkAddToGroupModal";
import { GameDetail } from "./components/GameDetail";
import { GameGrid } from "./components/GameGrid";
import { GroupAddModal } from "./components/GroupAddModal";
import { GroupNameModal } from "./components/GroupNameModal";
import { CustomSelect } from "./components/CustomSelect";
import { SettingsModal } from "./components/SettingsModal";
import { UpdateChecker } from "./components/UpdateChecker";
import { isTypingTarget, useGamepad, type PadAction } from "./hooks/useGamepad";
import {
  AppSettings,
  AppearancePrefs,
  applyAppearance,
  appearanceFromSettings,
  buildDefaultLibraryOrder,
  FILTER_OPTIONS,
  Game,
  GameGroup,
  gameOrderKey,
  groupOrderKey,
  isSortMode,
  LibraryFilter,
  LibraryStats,
  parseLibraryOrder,
  reconcileLibraryOrder,
  SORT_OPTIONS,
  SortMode,
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
    showTitles: true,
    showStoreLabels: true,
    gridDensity: "normal",
    coverCorners: "soft",
    coverShape: "portrait",
    reduceMotion: false,
  });
  const [stats, setStats] = useState<LibraryStats>({ total: 0, favorites: 0, missing: 0 });
  const [dataPath, setDataPath] = useState("");
  const [coverMap, setCoverMap] = useState<Record<string, string>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [groups, setGroups] = useState<GameGroup[]>([]);
  const [expandedGroupId, setExpandedGroupId] = useState<string | null>(null);
  const [libraryOrder, setLibraryOrder] = useState<string[]>([]);
  const [addToGroupTarget, setAddToGroupTarget] = useState<GameGroup | null>(null);
  const [addGameToGroupTarget, setAddGameToGroupTarget] = useState<Game | null>(null);
  const [groupNameModal, setGroupNameModal] = useState<
    null | { mode: "create" } | { mode: "rename"; group: GameGroup }
  >(null);
  const [selectMode, setSelectMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [bulkAddOpen, setBulkAddOpen] = useState(false);
  const [addMenuOpen, setAddMenuOpen] = useState(false);
  const [focusKey, setFocusKey] = useState<string | null>(null);
  const [focusables, setFocusables] = useState<string[]>([]);
  const [navActive, setNavActive] = useState(false);
  const coverLoaded = useRef<Set<string>>(new Set());
  const sessionStarts = useRef<Record<string, number>>({});
  const toastTimer = useRef<number | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const addMenuRef = useRef<HTMLDivElement>(null);
  const focusKeyRef = useRef<string | null>(null);
  const focusablesRef = useRef<string[]>([]);
  const navActiveRef = useRef(false);
  focusKeyRef.current = focusKey;
  focusablesRef.current = focusables;
  navActiveRef.current = navActive;

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
    applyAppearance(appearanceFromSettings(s));
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
    (nextOrder: string[], forceCustom = false) => {
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

  /** Snapshot visible order so switching to Custom doesn’t reshuffle games. */
  const captureCurrentOrder = useCallback(
    (extraGroup?: GameGroup | null) => {
      if (sortBy === "custom") {
        const next = [...libraryOrder];
        if (extraGroup) {
          const key = groupOrderKey(extraGroup.id);
          if (!next.includes(key)) next.unshift(key);
        }
        return next;
      }

      const grouped = new Set(groups.flatMap((g) => g.gameIds));
      const groupKeys = [...groups]
        .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name))
        .map((g) => groupOrderKey(g.id));
      if (extraGroup) {
        const key = groupOrderKey(extraGroup.id);
        if (!groupKeys.includes(key)) groupKeys.unshift(key);
      }
      const gameKeys = filtered
        .filter((g) => !grouped.has(g.id))
        .map((g) => gameOrderKey(g.id));
      return [...groupKeys, ...gameKeys];
    },
    [sortBy, libraryOrder, groups, filtered],
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
    persistLibraryOrder(captureCurrentOrder(group), sortBy === "custom");
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
      const seeded = captureCurrentOrder().filter((k) => k !== gameOrderKey(game.id));
      persistLibraryOrder(seeded, sortBy === "custom");
      showToast(`Added to “${group.name}”`);
      setAddGameToGroupTarget(null);
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
      persistLibraryOrder(next, sortBy === "custom");
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
      persistLibraryOrder(next, sortBy === "custom");
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

  const modalOpen =
    settingsOpen ||
    addToGroupTarget != null ||
    addGameToGroupTarget != null ||
    groupNameModal != null ||
    bulkAddOpen;

  const moveFocus = useCallback(
    (dx: number, dy: number) => {
      const keys = focusablesRef.current;
      if (keys.length === 0) return;
      const colsGuess = Math.max(
        1,
        Math.floor(
          ((document.querySelector(".game-grid") as HTMLElement | null)?.clientWidth ?? 600) /
            ((parseFloat(
              getComputedStyle(document.documentElement).getPropertyValue("--card-min"),
            ) || 150) +
              18),
        ),
      );
      setNavActive(true);
      let idx = focusKeyRef.current ? keys.indexOf(focusKeyRef.current) : -1;
      if (idx < 0) {
        setFocusKey(keys[0]);
        return;
      }
      if (dx !== 0) {
        idx = Math.max(0, Math.min(keys.length - 1, idx + dx));
      } else if (dy !== 0) {
        idx = Math.max(0, Math.min(keys.length - 1, idx + dy * colsGuess));
      }
      setFocusKey(keys[idx]);
    },
    [],
  );

  const activateFocus = useCallback(() => {
    const keys = focusablesRef.current;
    if (!navActiveRef.current || !focusKeyRef.current) {
      if (keys[0]) {
        setNavActive(true);
        setFocusKey(keys[0]);
      }
      return;
    }
    const key = focusKeyRef.current;
    if (key.startsWith("group:")) {
      const id = key.slice(6);
      setExpandedGroupId((cur) => (cur === id ? null : id));
      return;
    }
    if (key.startsWith("game:")) {
      const id = key.slice(5);
      const game = games.find((g) => g.id === id);
      if (!game) return;
      if (selectMode) {
        setSelectedIds((prev) => {
          const next = new Set(prev);
          if (next.has(id)) next.delete(id);
          else next.add(id);
          return next;
        });
      } else {
        setSelectedId(id);
        setNavActive(false);
      }
    }
  }, [games, selectMode]);

  const exitSelectMode = useCallback(() => {
    setSelectMode(false);
    setSelectedIds(new Set());
    setBulkAddOpen(false);
  }, []);

  const toggleSelectMode = useCallback(() => {
    setSelectMode((v) => {
      if (v) setSelectedIds(new Set());
      return !v;
    });
    setAddMenuOpen(false);
  }, []);

  const handleBulkFavorite = useCallback(async () => {
    const ids = [...selectedIds];
    for (const id of ids) {
      const g = games.find((x) => x.id === id);
      if (g && !g.favorite) await handleToggleFavorite(g);
    }
    showToast(`Favorited ${ids.length}`);
    exitSelectMode();
  }, [selectedIds, games, exitSelectMode, showToast]);

  const handleBulkHide = useCallback(async () => {
    const ids = [...selectedIds];
    for (const id of ids) {
      const g = games.find((x) => x.id === id);
      if (g && !g.hidden) await handleHide(g);
    }
    showToast(`Hidden ${ids.length}`);
    exitSelectMode();
  }, [selectedIds, games, exitSelectMode, showToast]);

  const handleBulkAddToGroup = useCallback(
    async (group: GameGroup) => {
      const ids = [...selectedIds];
      let n = 0;
      for (const id of ids) {
        const g = games.find((x) => x.id === id);
        if (!g || group.gameIds.includes(id)) continue;
        try {
          await invoke("add_game_to_group", { groupId: group.id, gameId: id });
          n++;
        } catch {
          /* skip failures */
        }
      }
      await refreshGroups();
      const seeded = captureCurrentOrder().filter((k) => !ids.includes(k.slice(5)));
      persistLibraryOrder(seeded, sortBy === "custom");
      showToast(`Added ${n} to “${group.name}”`);
      setBulkAddOpen(false);
      exitSelectMode();
    },
    [
      selectedIds,
      games,
      refreshGroups,
      captureCurrentOrder,
      persistLibraryOrder,
      sortBy,
      showToast,
      exitSelectMode,
    ],
  );

  const handlePadAction = useCallback(
    (action: PadAction) => {
      if (modalOpen) {
        if (action === "back") {
          setSettingsOpen(false);
          setAddToGroupTarget(null);
          setAddGameToGroupTarget(null);
          setGroupNameModal(null);
          setBulkAddOpen(false);
        }
        return;
      }
      if (selectedGame) {
        if (action === "back") setSelectedId(null);
        else if (action === "confirm") void handleLaunch(selectedGame);
        else if (action === "favorite") void handleToggleFavorite(selectedGame);
        return;
      }
      if (
        action === "up" ||
        action === "down" ||
        action === "left" ||
        action === "right" ||
        action === "confirm"
      ) {
        setNavActive(true);
      }
      if (action === "up") moveFocus(0, -1);
      else if (action === "down") moveFocus(0, 1);
      else if (action === "left") moveFocus(-1, 0);
      else if (action === "right") moveFocus(1, 0);
      else if (action === "confirm") activateFocus();
      else if (action === "back") {
        if (addMenuOpen) setAddMenuOpen(false);
        else if (navActiveRef.current) {
          setNavActive(false);
          setFocusKey(null);
        } else if (selectMode) exitSelectMode();
        else if (expandedGroupId) setExpandedGroupId(null);
        else if (query) setQuery("");
      } else if (action === "favorite") {
        const key = focusKeyRef.current;
        if (key?.startsWith("game:")) {
          const g = games.find((x) => x.id === key.slice(5));
          if (g) void handleToggleFavorite(g);
        }
      } else if (action === "select") toggleSelectMode();
    },
    [
      modalOpen,
      selectedGame,
      moveFocus,
      activateFocus,
      addMenuOpen,
      selectMode,
      exitSelectMode,
      expandedGroupId,
      query,
      games,
      toggleSelectMode,
    ],
  );

  const gamepadConnected = useGamepad(handlePadAction, !loading);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (modalOpen) {
        if (e.key === "Escape") {
          setSettingsOpen(false);
          setAddToGroupTarget(null);
          setAddGameToGroupTarget(null);
          setGroupNameModal(null);
          setBulkAddOpen(false);
        }
        return;
      }

      if (e.key === "Escape") {
        e.preventDefault();
        if (addMenuOpen) {
          setAddMenuOpen(false);
          return;
        }
        if (selectedGame) {
          setSelectedId(null);
          return;
        }
        if (selectMode) {
          exitSelectMode();
          return;
        }
        if (document.activeElement === searchRef.current && query) {
          setQuery("");
          return;
        }
        if (expandedGroupId) {
          setExpandedGroupId(null);
          return;
        }
        if (query) setQuery("");
        (document.activeElement as HTMLElement | null)?.blur?.();
        return;
      }

      if (isTypingTarget(e.target)) {
        if (e.key === "Escape") (e.target as HTMLElement).blur();
        return;
      }

      if (e.key === "/" || (e.key === "k" && (e.ctrlKey || e.metaKey))) {
        e.preventDefault();
        if (!selectedGame) searchRef.current?.focus();
        return;
      }

      if (selectedGame) {
        if (e.key === "Enter") {
          e.preventDefault();
          void handleLaunch(selectedGame);
        }
        return;
      }

      if (e.key === "s" || e.key === "S") {
        e.preventDefault();
        toggleSelectMode();
        return;
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        moveFocus(-1, 0);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        moveFocus(1, 0);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        moveFocus(0, -1);
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        moveFocus(0, 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        activateFocus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    addMenuOpen,
    selectedGame,
    selectMode,
    exitSelectMode,
    query,
    expandedGroupId,
    toggleSelectMode,
    moveFocus,
    activateFocus,
  ]);

  useEffect(() => {
    if (!addMenuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!addMenuRef.current?.contains(e.target as Node)) setAddMenuOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [addMenuOpen]);

  useEffect(() => {
    const clearNavOnMouse = (e: PointerEvent) => {
      if (!navActiveRef.current) return;
      if (e.pointerType && e.pointerType !== "mouse") return;
      setNavActive(false);
      setFocusKey(null);
    };
    window.addEventListener("pointerdown", clearNavOnMouse, true);
    return () => window.removeEventListener("pointerdown", clearNavOnMouse, true);
  }, []);

  useEffect(() => {
    if (!navActive) return;
    if (focusKey && focusables.includes(focusKey)) return;
    setFocusKey(focusables[0] ?? null);
    if (focusables.length === 0) setNavActive(false);
  }, [focusables, focusKey, navActive]);

  const showFocus = navActive && !selectedGame;

  return (
    <div className={`app-shell${gamepadConnected ? " gamepad-connected" : ""}`}>
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
                ref={searchRef}
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search…  /"
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
          </div>
        )}
        <div className="top-actions">
          {!selectedGame && (
            <>
              <div className="icon-menu" ref={addMenuRef}>
                <button
                  type="button"
                  className={`icon-btn${addMenuOpen ? " active" : ""}`}
                  aria-label="Add"
                  data-tip="Add"
                  aria-expanded={addMenuOpen}
                  aria-haspopup="menu"
                  onClick={() => setAddMenuOpen((v) => !v)}
                >
                  <svg viewBox="0 0 24 24" aria-hidden>
                    <path
                      fill="currentColor"
                      d="M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6V5z"
                    />
                  </svg>
                </button>
                {addMenuOpen && (
                  <div className="icon-menu-panel" role="menu">
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => {
                        setAddMenuOpen(false);
                        void handleAddManual();
                      }}
                    >
                      Game
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => {
                        setAddMenuOpen(false);
                        void handleCreateGroup();
                      }}
                    >
                      Group
                    </button>
                  </div>
                )}
              </div>
              <button
                type="button"
                className={`icon-btn${selectMode ? " active" : ""}`}
                aria-label={selectMode ? "Exit select" : "Select"}
                data-tip="Select"
                aria-pressed={selectMode}
                onClick={toggleSelectMode}
              >
                <svg viewBox="0 0 24 24" aria-hidden>
                  <path
                    fill="currentColor"
                    d="M9 3h6v2H9V3zm-4 4h14v2H5V7zm2 4h10v2H7v-2zm2 4h6v2H9v-2zm-2 4h10v2H7v-2z"
                  />
                </svg>
              </button>
            </>
          )}
          <button
            type="button"
            className="icon-btn"
            aria-label={scanning ? "Scanning" : "Rescan"}
            data-tip={scanning ? "Scanning…" : "Rescan"}
            onClick={handleRescan}
            disabled={scanning}
          >
            <svg viewBox="0 0 24 24" aria-hidden className={scanning ? "spin" : undefined}>
              <path
                fill="currentColor"
                d="M12 6V3L8 7l4 4V8c2.76 0 5 2.24 5 5a5 5 0 0 1-8.9 3.1L6.7 17.5A7 7 0 0 0 19 13c0-3.87-3.13-7-7-7zm0 12v3l4-4-4-4v3a5 5 0 0 1-5-5c0-1.1.36-2.12.97-2.95l-1.4-1.41A7 7 0 0 0 5 13c0 3.87 3.13 7 7 7z"
              />
            </svg>
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="Settings"
            data-tip="Settings"
            onClick={() => setSettingsOpen(true)}
          >
            <svg viewBox="0 0 24 24" aria-hidden>
              <path
                fill="currentColor"
                d="M19.14 12.94c.04-.31.06-.63.06-.94s-.02-.63-.06-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.1 7.1 0 0 0-1.63-.94l-.36-2.54A.5.5 0 0 0 13.9 2h-3.8a.5.5 0 0 0-.49.42l-.36 2.54c-.59.24-1.13.55-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.71 8.48a.5.5 0 0 0 .12.64l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94L2.83 14.58a.5.5 0 0 0-.12.64l1.92 3.32c.14.24.43.34.68.22l2.39-.96c.5.39 1.04.71 1.63.94l.36 2.54c.05.24.25.42.49.42h3.8c.24 0 .44-.18.49-.42l.36-2.54c.59-.24 1.13-.55 1.63-.94l2.39.96c.25.12.54.02.68-.22l1.92-3.32a.5.5 0 0 0-.12-.64l-2.03-1.58zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
              />
            </svg>
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
                onExpandedGroupChange={(id) => {
                  setNavActive(false);
                  setExpandedGroupId(id);
                }}
                focusKey={showFocus ? focusKey : null}
                selectMode={selectMode}
                selectedIds={selectedIds}
                onToggleSelect={(g) => {
                  setSelectedIds((prev) => {
                    const next = new Set(prev);
                    if (next.has(g.id)) next.delete(g.id);
                    else next.add(g.id);
                    return next;
                  });
                  setFocusKey(`game:${g.id}`);
                }}
                onFocusablesChange={setFocusables}
                onOpen={(g) => {
                  setNavActive(false);
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
                onRequestAddToGroup={(g) => setAddGameToGroupTarget(g)}
                onCreateGroup={handleCreateGroup}
                onRemoveFromGroup={handleRemoveFromGroup}
                onReorder={(next) => persistLibraryOrder(next, true)}
                onAddGamesToGroup={(g) => setAddToGroupTarget(g)}
              />
            )}
          </>
        )}
      </main>

      {selectMode && !selectedGame && (
        <div className="bulk-bar" role="toolbar" aria-label="Bulk actions">
          <span className="bulk-count">{selectedIds.size} selected</span>
          <button
            type="button"
            className="btn"
            disabled={selectedIds.size === 0}
            onClick={() => void handleBulkFavorite()}
          >
            Favorite
          </button>
          <button
            type="button"
            className="btn"
            disabled={selectedIds.size === 0}
            onClick={() => void handleBulkHide()}
          >
            Hide
          </button>
          <button
            type="button"
            className="btn"
            disabled={selectedIds.size === 0}
            onClick={() => setBulkAddOpen(true)}
          >
            Add to group…
          </button>
          <button type="button" className="btn" onClick={exitSelectMode}>
            Cancel
          </button>
        </div>
      )}

      {gamepadConnected && !selectedGame && !modalOpen && (
        <div className="gamepad-hint" aria-hidden>
          <span>D-pad move</span>
          <span>A open</span>
          <span>B back</span>
          <span>X favorite</span>
          <span>View select</span>
        </div>
      )}

      <BulkAddToGroupModal
        open={bulkAddOpen}
        gameCount={selectedIds.size}
        groups={groups}
        onClose={() => setBulkAddOpen(false)}
        onPick={(group) => void handleBulkAddToGroup(group)}
        onCreateGroup={() => {
          setBulkAddOpen(false);
          void handleCreateGroup();
        }}
      />

      <AddToGroupModal
        open={addGameToGroupTarget != null}
        game={addGameToGroupTarget}
        groups={groups}
        onClose={() => setAddGameToGroupTarget(null)}
        onPick={(group) => {
          if (addGameToGroupTarget) void handleAddToGroup(addGameToGroupTarget, group);
        }}
        onCreateGroup={handleCreateGroup}
      />

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
        onPreviewAppearance={(prefs: AppearancePrefs) => {
          applyAppearance(prefs);
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
