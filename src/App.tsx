import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AddToGroupModal } from "./components/AddToGroupModal";
import { BulkAddToGroupModal } from "./components/BulkAddToGroupModal";
import { CoverPickModal } from "./components/CoverPickModal";
import { FlyCover, type FlyOrigin } from "./components/FlyCover";
import { GameDetail } from "./components/GameDetail";
import { GameGrid } from "./components/GameGrid";
import { GroupAddModal } from "./components/GroupAddModal";
import { GroupNameModal } from "./components/GroupNameModal";
import { CustomSelect } from "./components/CustomSelect";
import { MergeModal } from "./components/MergeModal";
import { SettingsModal } from "./components/SettingsModal";
import { StatsPage } from "./components/StatsPage";
import { SystemPage } from "./components/SystemPage";
import { UpdateChecker } from "./components/UpdateChecker";
import { isTypingTarget, useGamepad, type PadAction } from "./hooks/useGamepad";
import {
  AppSettings,
  AppearancePrefs,
  applyAppearance,
  appearanceFromSettings,
  buildDefaultLibraryOrder,
  CoverChoiceGroup,
  coverSrc,
  DuplicateGroup,
  Game,
  GameGroup,
  gameOrderKey,
  groupOrderKey,
  isSortMode,
  LibraryFilter,
  LibraryOverview,
  LibraryStats,
  localCoverUrl,
  asLibraryGame,
  MainView,
  neverPlayed,
  parseLibraryOrder,
  QUICK_FILTERS,
  reconcileLibraryOrder,
  SORT_OPTIONS,
  SortMode,
  STORE_FILTER_OPTIONS,
} from "./types";
import "./styles/theme.css";
import "./styles/App.css";

const COVER_PICK_KEY = "intelgen.coverPickDismissed";

type CoverUpdatedPayload = {
  id: string;
  coverPath: string;
  steamAppId: string | null;
  coverUrl: string | null;
  genre: string | null;
  logoPath: string | null;
};

function coverPickSignature(group: CoverChoiceGroup): string {
  return [...group.paths]
    .map((p) => p.replace(/\\/g, "/").toLowerCase())
    .sort()
    .join("\n");
}

function loadCoverPickDismissed(): Record<string, string> {
  try {
    const raw = localStorage.getItem(COVER_PICK_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    return parsed && typeof parsed === "object" ? (parsed as Record<string, string>) : {};
  } catch {
    return {};
  }
}

function rememberCoverPick(group: CoverChoiceGroup) {
  const map = loadCoverPickDismissed();
  map[group.gameId] = coverPickSignature(group);
  localStorage.setItem(COVER_PICK_KEY, JSON.stringify(map));
}

function filterCoverPickQueue(groups: CoverChoiceGroup[]): CoverChoiceGroup[] {
  const dismissed = loadCoverPickDismissed();
  return groups.filter((g) => g.paths.length >= 2 && dismissed[g.gameId] !== coverPickSignature(g));
}

function App() {
  const [games, setGames] = useState<Game[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [query, setQuery] = useState("");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sortBy, setSortBy] = useState<SortMode>("name");
  const [toast, setToast] = useState<{
    text: string;
    error?: boolean;
    undo?: () => void;
  } | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [systemOpen, setSystemOpen] = useState(false);
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
    startWithWindows: false,
    closeToTray: false,
    startInBackground: false,
  });
  const [stats, setStats] = useState<LibraryStats>({ total: 0, favorites: 0, missing: 0 });
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
  const [mainView, setMainView] = useState<MainView>("library");
  const [storeFlyout, setStoreFlyout] = useState(false);
  const [overview, setOverview] = useState<LibraryOverview | null>(null);
  const [overviewLoading, setOverviewLoading] = useState(false);
  const [mergeOpen, setMergeOpen] = useState(false);
  const [dupGroups, setDupGroups] = useState<DuplicateGroup[]>([]);
  const [coverPickQueue, setCoverPickQueue] = useState<CoverChoiceGroup[]>([]);
  const [fly, setFly] = useState<FlyOrigin | null>(null);
  const [flyDone, setFlyDone] = useState(true);
  const flyTimer = useRef<number>(0);
  const libraryScrollRef = useRef(0);
  const [bigPicture, setBigPicture] = useState(false);
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

  const showToast = useCallback((text: string, error = false, undo?: () => void) => {
    setToast({ text, error, undo });
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), undo ? 6500 : 3200);
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

  const loadCoverPickQueue = useCallback(async () => {
    try {
      const groups = await invoke<CoverChoiceGroup[]>("list_cover_choice_groups");
      const incoming = filterCoverPickQueue(groups);
      setCoverPickQueue((prev) => {
        if (prev.length === 0) return incoming;
        const have = new Set(prev.map((g) => g.gameId));
        return [...prev, ...incoming.filter((g) => !have.has(g.gameId))];
      });
    } catch {
      /* ignore */
    }
  }, []);

  const bootstrap = useCallback(async () => {
    setLoading(true);
    try {
      const s = await invoke<AppSettings>("get_settings");
      setSettings(s);
      if (isSortMode(s.sortBy)) setSortBy(s.sortBy);
      applySettingsAppearance(s);

      let list = await invoke<Game[]>("list_games");
      setGames(list.map(asLibraryGame));
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
        setGames(list.map(asLibraryGame));
        setScanning(false);
      }
      await refreshStats();

      invoke("fetch_covers", { ids: null }).catch(() => undefined);
      void loadCoverPickQueue();
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setLoading(false);
      setScanning(false);
    }
  }, [applySettingsAppearance, loadCoverPickQueue, refreshStats, showToast]);

  useEffect(() => {
    bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    if (selectedId || mainView !== "library") return;
    const main = document.querySelector(".main");
    if (!main) return;
    const onScroll = () => {
      libraryScrollRef.current = (main as HTMLElement).scrollTop;
    };
    main.addEventListener("scroll", onScroll, { passive: true });
    return () => main.removeEventListener("scroll", onScroll);
  }, [selectedId, mainView]);

  useEffect(() => {
    const main = document.querySelector(".main") as HTMLElement | null;
    if (!main) return;
    if (selectedId) {
      main.scrollTop = 0;
      return;
    }
    if (mainView !== "library") return;
    const y = libraryScrollRef.current;
    const restore = () => {
      main.scrollTop = y;
    };
    restore();
    const raf = requestAnimationFrame(() => {
      restore();
      requestAnimationFrame(restore);
    });
    const t = window.setTimeout(restore, 80);
    return () => {
      cancelAnimationFrame(raf);
      window.clearTimeout(t);
    };
  }, [selectedId, mainView]);

  useEffect(() => {
    let cancelled = false;
    let unlistenCover: (() => void) | undefined;
    let unlistenDone: (() => void) | undefined;
    const pending = new Map<string, CoverUpdatedPayload>();
    let raf = 0;

    const flush = () => {
      raf = 0;
      if (pending.size === 0) return;
      const batch = [...pending.values()];
      pending.clear();
      setGames((prev) => {
        const byId = new Map(batch.map((p) => [p.id, p]));
        return prev.map((g) => {
          const p = byId.get(g.id);
          if (!p) return g;
          return {
            ...g,
            coverPath: p.coverPath || g.coverPath,
            steamAppId: p.steamAppId ?? g.steamAppId,
            coverUrl: p.coverUrl ?? g.coverUrl,
            genre: p.genre ?? g.genre,
            logoPath: p.logoPath ?? g.logoPath,
          };
        });
      });
      setCoverMap((m) => {
        const next = { ...m };
        for (const p of batch) {
          if (!p.coverPath) continue;
          const url = localCoverUrl(p.coverPath);
          if (url) next[p.id] = url;
        }
        return next;
      });
    };

    (async () => {
      const { listen } = await import("@tauri-apps/api/event");
      unlistenCover = await listen<CoverUpdatedPayload>("cover-updated", (event) => {
        pending.set(event.payload.id, event.payload);
        if (!raf) raf = window.requestAnimationFrame(flush);
      });
      unlistenDone = await listen("covers-done", () => {
        if (!cancelled) void loadCoverPickQueue();
      });
    })();
    return () => {
      cancelled = true;
      if (raf) window.cancelAnimationFrame(raf);
      flush();
      unlistenCover?.();
      unlistenDone?.();
    };
  }, [loadCoverPickQueue]);

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
          if (!cancelled) setGames(hidden.map(asLibraryGame));
        } else {
          const list = await invoke<Game[]>("list_games");
          if (!cancelled) setGames(list.map(asLibraryGame));
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
      if (libraryFilter === "never" && !neverPlayed(g)) return false;
      if (libraryFilter === "missing" && !g.missing) return false;
      if (libraryFilter === "other") {
        if (g.store !== "manual" && g.store !== "roblox") return false;
      } else if (
        libraryFilter !== "all" &&
        libraryFilter !== "favorites" &&
        libraryFilter !== "never" &&
        libraryFilter !== "missing" &&
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

  /** Snapshot visible order so switching to Custom doesn't reshuffle games. */
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

  async function promptDuplicates() {
    try {
      const groups = await invoke<DuplicateGroup[]>("suggest_duplicates");
      if (groups.length === 0) return;
      setDupGroups(groups);
      setMergeOpen(true);
    } catch {
      /* ignore */
    }
  }

  async function handleRescan() {
    setScanning(true);
    try {
      const list = await invoke<Game[]>("rescan_library");
      setGames(list.map(asLibraryGame));
      await refreshStats();
      showToast(`Library updated · ${list.length} games`);
      invoke("fetch_covers", { ids: null }).catch(() => undefined);
      await promptDuplicates();
      void loadCoverPickQueue();
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setScanning(false);
    }
  }

  function skipCoverPick() {
    const group = coverPickQueue[0];
    if (group) rememberCoverPick(group);
    setCoverPickQueue((prev) => prev.slice(1));
  }

  async function handleCoverPick(path: string) {
    const group = coverPickQueue[0];
    if (!group) return;
    try {
      const updated = await invoke<Game>("choose_cover", { id: group.gameId, path });
      setGames((prev) => prev.map((g) => (g.id === updated.id ? asLibraryGame({ ...g, ...updated }) : g)));
      if (updated.coverPath) {
        const url = localCoverUrl(updated.coverPath, true);
        if (url) setCoverMap((m) => ({ ...m, [updated.id]: url }));
      }
      rememberCoverPick(group);
      setCoverPickQueue((prev) => prev.slice(1));
    } catch (e) {
      showToast(String(e), true);
    }
  }

  const addManualFromPath = useCallback(
    async (path: string, groupId?: string | null) => {
      const game = await invoke<Game>("add_manual_game", { path });
      setGames((prev) => {
        const rest = prev.filter((g) => g.id !== game.id);
        return [...rest, asLibraryGame(game)].sort((a, b) => a.name.localeCompare(b.name));
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
        showToast(`Added ${game.name} to "${group.name}"`);
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
        const minutes = Math.min(12 * 60, Math.max(0, Math.round((Date.now() - prev) / 60000)));
        await invoke("end_play_session", { id: game.id, minutes });
      }
      sessionStarts.current[game.id] = Date.now();
      const updated = await invoke<Game>("launch_game", { id: game.id });
      setGames((prev) =>
        prev.map((g) =>
          g.id === updated.id
            ? asLibraryGame({ ...updated, playtimeMinutes: g.playtimeMinutes })
            : g,
        ),
      );
      showToast(`Launching ${game.name}`);
    } catch (e) {
      showToast(String(e), true);
    }
  }

  useEffect(() => {
    const onFocus = async () => {
      for (const [id, started] of Object.entries(sessionStarts.current)) {
        const minutes = Math.min(12 * 60, Math.round((Date.now() - started) / 60000));
        if (minutes >= 1) {
          try {
            const updated = await invoke<Game>("end_play_session", {
              id,
              minutes,
            });
            setGames((prev) => prev.map((g) => (g.id === updated.id ? asLibraryGame(updated) : g)));
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
      setGames((prev) => prev.map((g) => (g.id === updated.id ? asLibraryGame(updated) : g)));
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
          setGames((prev) => prev.map((g) => (g.id === updated.id ? asLibraryGame(updated) : g)));
        }
      } else if (hide) {
        setGames((prev) => prev.filter((g) => g.id !== game.id));
        if (selectedId === game.id) setSelectedId(null);
      }
      await refreshStats();
      if (hide) {
        showToast(`Hidden ${game.name}`, false, () => {
          void (async () => {
            const restored = await invoke<Game>("set_hidden", { id: game.id, hidden: false });
            setGames((prev) =>
              prev.some((g) => g.id === restored.id)
                ? prev.map((g) => (g.id === restored.id ? asLibraryGame(restored) : g))
                : [...prev, asLibraryGame(restored)],
            );
            await refreshStats();
          })();
        });
      } else {
        showToast(`Restored ${game.name}`);
      }
    } catch (e) {
      showToast(String(e), true);
    }
  }

  async function handleRemove(game: Game) {
    try {
      const snapshot = await invoke<Game>("remove_game", { id: game.id });
      setGames((prev) => prev.filter((g) => g.id !== game.id));
      setCoverMap((m) => {
        const next = { ...m };
        delete next[game.id];
        return next;
      });
      if (selectedId === game.id) setSelectedId(null);
      await refreshStats();
      let undone = false;
      showToast(`Removed ${game.name}`, false, () => {
        undone = true;
        void (async () => {
          const restored = await invoke<Game>("restore_game", { game: snapshot });
          setGames((prev) => [...prev.filter((g) => g.id !== restored.id), asLibraryGame(restored)]);
          await refreshStats();
        })();
      });
      window.setTimeout(() => {
        if (!undone) invoke("finalize_remove", { id: game.id }).catch(() => undefined);
      }, 7000);
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

  async function handleOpenSaveFolder(game: Game) {
    try {
      await invoke("launch_action", { id: game.id, action: "save_folder" });
    } catch (e) {
      showToast(String(e), true);
    }
  }

  function captureLibraryScroll() {
    if (selectedId || mainView !== "library") return;
    const main = document.querySelector(".main") as HTMLElement | null;
    if (main) libraryScrollRef.current = main.scrollTop;
  }

  function openGame(game: Game) {
    captureLibraryScroll();
    const reduce = document.body.classList.contains("reduce-motion");
    const tile = document.querySelector(
      `[data-focus-key="game:${game.id.replace(/"/g, '\\"')}"] .cover`,
    ) as HTMLElement | null;
    const from = tile?.getBoundingClientRect();
    window.clearTimeout(flyTimer.current);
    if (tile && from && from.width > 8 && !reduce) {
      const img = tile.querySelector("img") as HTMLImageElement | null;
      setFly({
        from,
        src: img?.currentSrc || img?.src || coverSrc(game) || null,
        name: game.name,
        game,
      });
      setFlyDone(false);
      flyTimer.current = window.setTimeout(() => setFlyDone(true), 600);
    } else {
      setFly(null);
      setFlyDone(true);
    }
    setMainView("library");
    setSelectedId(game.id);
    setNavActive(false);
  }

  function handleRandomLaunch() {
    const pool = filtered.filter((g) => !g.missing);
    if (pool.length === 0) {
      showToast("No launchable games in this view", true);
      return;
    }
    const pick = pool[Math.floor(Math.random() * pool.length)];
    void handleLaunch(pick);
  }

  async function toggleBigPicture() {
    const win = getCurrentWindow();
    const next = !bigPicture;
    setBigPicture(next);
    try {
      await win.setFullscreen(next);
    } catch {
      /* ignore */
    }
    if (next) {
      setMainView("library");
      setSelectedId(null);
      setNavActive(true);
    }
  }

  async function loadOverview() {
    setOverviewLoading(true);
    try {
      const data = await invoke<LibraryOverview>("library_overview");
      setOverview(data);
    } catch (e) {
      showToast(String(e), true);
    } finally {
      setOverviewLoading(false);
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
    showToast(`Created "${group.name}"`);
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
      showToast(`Added to "${group.name}"`);
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
    if (!confirm(`Delete group "${group.name}"? Games stay in your library.`)) return;
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
      showToast(`Deleted "${group.name}"`);
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
      showToast(`Removed from "${group.name}"`);
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
    bulkAddOpen ||
    mergeOpen ||
    coverPickQueue.length > 0;

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
      showToast(`Added ${n} to "${group.name}"`);
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
          setMergeOpen(false);
        }
        return;
      }

      if (e.key === "Escape") {
        e.preventDefault();
        if (addMenuOpen) {
          setAddMenuOpen(false);
          return;
        }
        if (storeFlyout) {
          setStoreFlyout(false);
          return;
        }
        if (selectedGame) {
          setSelectedId(null);
          setFly(null);
          setFlyDone(true);
          return;
        }
        if (bigPicture) {
          void toggleBigPicture();
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

  useEffect(() => {
    if (mainView === "stats") void loadOverview();
  }, [mainView]);

  useEffect(() => {
    if (!storeFlyout) return;
    const onDoc = (e: MouseEvent) => {
      const el = document.querySelector(".store-flyout");
      if (el && !el.contains(e.target as Node)) setStoreFlyout(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [storeFlyout]);

  const storeFilterActive = STORE_FILTER_OPTIONS.some((o) => o.id === libraryFilter);
  const emptyCopy = (() => {
    if (!loading && games.length === 0) {
      return {
        title: "No games found",
        body: "Install a store library or add a game manually, then hit Rescan.",
      };
    }
    if (query.trim()) {
      return { title: "No matches", body: `Nothing named "${query.trim()}" in this view.` };
    }
    if (libraryFilter === "favorites") {
      return { title: "No favorites yet", body: "Star a game on its tile to pin it here." };
    }
    if (libraryFilter === "never") {
      return { title: "All caught up", body: "Every game in this view has been played at least once." };
    }
    if (libraryFilter === "missing") {
      return { title: "Nothing missing", body: "All scanned games still look installed." };
    }
    if (libraryFilter === "hidden") {
      return { title: "No hidden games", body: "Hidden titles stay out of the main grid until you restore them." };
    }
    if (storeFilterActive) {
      return { title: "No games from this store", body: "Rescan, or pick another source in Stores." };
    }
    return { title: "No games match", body: "Try clearing filters or running a rescan." };
  })();

  return (
    <div
      className={`app-shell${gamepadConnected ? " gamepad-connected" : ""}${bigPicture ? " big-picture" : ""}`}
    >
      <aside className="sidebar" aria-label="Main">
        <div className="brand sidebar-brand">
          <img src="/intelgen-icon.png" alt="" className="brand-mark" />
          <h1 className="brand-name">IntelGen</h1>
        </div>
        <nav className="sidebar-nav">
          <button
            type="button"
            className={`sidebar-btn${mainView === "library" && !selectedGame ? " active" : ""}`}
            onClick={() => {
              setMainView("library");
              setSelectedId(null);
            }}
          >
            Library
          </button>
          <button
            type="button"
            className={`sidebar-btn${mainView === "stats" ? " active" : ""}`}
            onClick={() => {
              captureLibraryScroll();
              setSelectedId(null);
              setMainView("stats");
            }}
          >
            Stats
          </button>
        </nav>
        <div className="sidebar-foot">
          <button
            type="button"
            className={`sidebar-btn${systemOpen ? " active" : ""}`}
            onClick={() => setSystemOpen(true)}
          >
            System
          </button>
          <button
            type="button"
            className={`sidebar-btn${bigPicture ? " active" : ""}`}
            onClick={() => void toggleBigPicture()}
          >
            Big Picture
          </button>
          <button type="button" className="sidebar-btn" onClick={() => setSettingsOpen(true)}>
            Settings
          </button>
        </div>
      </aside>

      <div className="app-col">
        <header className="topbar">
          {mainView === "library" && !selectedGame && (
            <div className="topbar-tools">
              <div className="search-wrap">
                <span className="icon" aria-hidden>
                  <svg viewBox="0 0 24 24">
                    <path
                      fill="currentColor"
                      d="M15.5 14h-.79l-.28-.27A6.47 6.47 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14"
                    />
                  </svg>
                </span>
                <input
                  ref={searchRef}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="Search"
                  aria-label="Search games"
                />
              </div>
              <div className="filter-chips" role="tablist" aria-label="Quick filters">
                {QUICK_FILTERS.map((f) => (
                  <button
                    key={f.id}
                    type="button"
                    className={`chip-btn${libraryFilter === f.id ? " active" : ""}`}
                    onClick={() => setLibraryFilter(f.id)}
                  >
                    {f.label}
                  </button>
                ))}
                <div className="store-flyout">
                  <button
                    type="button"
                    className={`chip-btn${storeFlyout || storeFilterActive ? " active" : ""}`}
                    onClick={() => setStoreFlyout((v) => !v)}
                  >
                    {storeFilterActive
                      ? (STORE_FILTER_OPTIONS.find((s) => s.id === libraryFilter)?.label ?? "Stores")
                      : "Stores"}
                  </button>
                  {storeFlyout && (
                    <div className="icon-menu-panel store-menu" role="menu">
                      {STORE_FILTER_OPTIONS.map((s) => (
                        <button
                          key={s.id}
                          type="button"
                          role="menuitem"
                          className={libraryFilter === s.id ? "active" : ""}
                          onClick={() => {
                            setLibraryFilter(s.id);
                            setStoreFlyout(false);
                          }}
                        >
                          {s.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              </div>
              <CustomSelect
                value={sortBy}
                options={SORT_OPTIONS}
                onChange={persistSort}
                ariaLabel="Sort games"
              />
            </div>
          )}
          <div className="top-actions">
            {mainView === "library" && !selectedGame && (
              <>
                <button
                  type="button"
                  className="icon-btn"
                  aria-label="Random game"
                  data-tip="Random"
                  onClick={handleRandomLaunch}
                >
                  <svg viewBox="0 0 24 24" aria-hidden>
                    <path
                      fill="currentColor"
                      d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zM7.5 18c-.83 0-1.5-.67-1.5-1.5S6.67 15 7.5 15s1.5.67 1.5 1.5S8.33 18 7.5 18zm0-9C6.67 9 6 8.33 6 7.5S6.67 6 7.5 6 9 6.67 9 7.5 8.33 9 7.5 9zm4.5 4.5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm4.5 4.5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm0-9c-.83 0-1.5-.67-1.5-1.5S15.67 6 16.5 6s1.5.67 1.5 1.5S17.33 9 16.5 9z"
                    />
                  </svg>
                </button>
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
              data-tip={scanning ? "Scanning..." : "Rescan"}
              onClick={() => void handleRescan()}
              disabled={scanning}
            >
              <svg viewBox="0 0 24 24" aria-hidden className={scanning ? "spin" : undefined}>
                <path
                  fill="currentColor"
                  d="M12 6V3L8 7l4 4V8c2.76 0 5 2.24 5 5a5 5 0 0 1-8.9 3.1L6.7 17.5A7 7 0 0 0 19 13c0-3.87-3.13-7-7-7zm0 12v3l4-4-4-4v3a5 5 0 0 1-5-5c0-1.1.36-2.12.97-2.95l-1.4-1.41A7 7 0 0 0 5 13c0 3.87 3.13 7 7 7z"
                />
              </svg>
            </button>
          </div>
        </header>

        <main className="main">
          <div
            className="library-home"
            data-library-hidden={selectedGame || mainView !== "library" ? "true" : undefined}
            hidden={!!selectedGame || mainView !== "library"}
          >
              <div className="status-bar">
                <div>
                  {loading || scanning ? (
                    <span className="loading-pulse">
                      <span className="dot" />
                      {scanning ? "Scanning libraries..." : "Loading library..."}
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
                <div className="empty empty-filter">
                  <div className="empty-art" aria-hidden>
                    <span />
                    <span />
                    <span />
                  </div>
                  <h2>{emptyCopy.title}</h2>
                  <p>{emptyCopy.body}</p>
                  <button type="button" className="btn btn-primary" onClick={() => void handleRescan()}>
                    Rescan library
                  </button>
                </div>
              ) : (
                <GameGrid
                  games={filtered}
                  groups={groups}
                  coverMap={coverMap}
                  emptyTitle={emptyCopy.title}
                  emptyBody={emptyCopy.body}
                  active={!selectedGame && mainView === "library"}
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
                  onOpen={openGame}
                  onLaunch={handleLaunch}
                  onOpenSaveFolder={handleOpenSaveFolder}
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
          </div>
          {selectedGame ? (
            <GameDetail
              game={selectedGame}
              coverDataUrl={coverMap[selectedGame.id]}
              coverHidden={!flyDone}
              onBack={() => {
                setSelectedId(null);
                setFly(null);
                setFlyDone(true);
              }}
              onLaunch={handleLaunch}
              onOpenSaveFolder={handleOpenSaveFolder}
              onToggleFavorite={handleToggleFavorite}
              onRemove={handleRemove}
              onGameUpdated={(g) =>
                setGames((prev) =>
                  prev.map((x) => (x.id === g.id ? asLibraryGame({ ...x, ...g }) : x)),
                )
              }
              onCoverUpdated={(id, dataUrl) => {
                setCoverMap((m) => ({ ...m, [id]: dataUrl }));
              }}
            />
          ) : mainView === "stats" ? (
            <StatsPage
              overview={overview}
              loading={overviewLoading}
              onOpenGame={(id) => {
                const g = games.find((x) => x.id === id);
                if (g) openGame(g);
              }}
            />
          ) : null}
        </main>
      </div>

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
            Add to group...
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

      {fly && !flyDone && (
        <FlyCover
          fly={fly}
          onDone={() => {
            setFly(null);
            setFlyDone(true);
          }}
        />
      )}

      {coverPickQueue[0] && !settingsOpen && !mergeOpen && !selectedId && (
        <CoverPickModal
          group={coverPickQueue[0]}
          index={0}
          total={coverPickQueue.length}
          onPick={(path) => void handleCoverPick(path)}
          onSkip={skipCoverPick}
        />
      )}

      <MergeModal
        open={mergeOpen}
        games={
          selectedIds.size >= 2
            ? games.filter((g) => selectedIds.has(g.id))
            : []
        }
        groups={dupGroups}
        coverMap={coverMap}
        onClose={() => setMergeOpen(false)}
        onMerge={async (keepId, sourceIds) => {
          const kept = await invoke<Game>("merge_games", { keepId, sourceIds });
          setGames((prev) => {
            const drop = new Set(sourceIds);
            return [...prev.filter((g) => !drop.has(g.id) && g.id !== keepId), asLibraryGame(kept)];
          });
          exitSelectMode();
          await refreshStats();
          showToast(`Merged into ${kept.name}`);
        }}
      />

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

      <SystemPage open={systemOpen} onClose={() => setSystemOpen(false)} />

      <SettingsModal
        open={settingsOpen}
        settings={settings}
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
        onResetArt={async () => {
          await invoke("reset_all_art");
          setCoverMap({});
          setSelectedId(null);
          const list = await invoke<Game[]>("list_games");
          setGames(list.map(asLibraryGame));
          invoke("fetch_covers", { ids: null }).catch(() => undefined);
          showToast("Cover art reset — fetching new art");
        }}
        onResetStats={async () => {
          await invoke("reset_all_stats");
          sessionStarts.current = {};
          const list = await invoke<Game[]>("list_games");
          setGames(list.map(asLibraryGame));
          await refreshStats();
          if (mainView === "stats") await loadOverview();
          else setOverview(null);
          showToast("Stats reset");
        }}
      />

      <UpdateChecker />

      {toast && (
        <div className={`toast${toast.error ? " error" : ""}`} role="status">
          <span>{toast.text}</span>
          {toast.undo && (
            <button
              type="button"
              className="toast-undo"
              onClick={() => {
                toast.undo?.();
                setToast(null);
              }}
            >
              Undo
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
