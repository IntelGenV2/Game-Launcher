import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { isTypingTarget, useGamepad, type PadAction } from "../hooks/useGamepad";
import {
  isBpMuted,
  playBack,
  playBoot,
  playConfirm,
  playError,
  playMove,
  playPower,
  playReady,
  setBpMuted,
} from "../sounds";
import {
  Game,
  GameGroup,
  GameStats,
  LibraryFilter,
  LibraryOverview,
  QUICK_FILTERS,
  SORT_OPTIONS,
  STORE_FILTER_OPTIONS,
  STORE_LABELS,
  SortMode,
  Store,
  coverSourceLabel,
  coverSrc,
  formatLastPlayed,
  formatPlaytime,
  gameOrderKey,
  neverPlayed,
} from "../types";
import { BarChart } from "./Charts";
import { CoverImg } from "./CoverImg";
import type { SystemInfo } from "./SystemPage";

type Screen = "library" | "explorer" | "stats" | "system";
type Zone = "tabs" | "stage" | "list";
type ConfirmKind = "restart" | "shutdown" | "exit";
type GameTab = "overview" | "sessions";
type StripItem =
  | { kind: "folder"; id: string; group: GameGroup; members: Game[] }
  | { kind: "game"; id: string; game: Game };

interface ExplorerPlace {
  id: string;
  name: string;
  path: string | null;
}
interface ExplorerEntry {
  name: string;
  path: string;
  isDir: boolean;
  kind: string;
}
interface ExplorerListing {
  path: string | null;
  parent: string | null;
  label: string;
  entries: ExplorerEntry[];
}

const TABS: { id: Screen; label: string; icon: string }[] = [
  { id: "library", label: "Library", icon: "▣" },
  { id: "explorer", label: "Files", icon: "▤" },
  { id: "stats", label: "Stats", icon: "▦" },
  { id: "system", label: "System", icon: "⏻" },
];
const BP_FILTERS = [...QUICK_FILTERS, ...STORE_FILTER_OPTIONS.filter((s) => s.id !== "hidden")];
const BP_SORTS = SORT_OPTIONS.filter((s) => s.id !== "custom");
const BOOT_MS = 2400;
const BOOT_MS_FAST = 360;
const FLOW_WINDOW = 5;

function clockParts(d: Date) {
  return {
    time: d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }),
    date: d.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" }),
  };
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes < 0) return "—";
  const gb = bytes / 1024 ** 3;
  if (gb >= 10) return `${gb.toFixed(0)} GB`;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
}

function kindGlyph(kind: string): string {
  switch (kind) {
    case "drive":
      return "▣";
    case "folder":
      return "📁";
    case "app":
      return "▶";
    default:
      return "·";
  }
}

function filterAndSort(
  games: Game[],
  query: string,
  libraryFilter: LibraryFilter,
  sortBy: SortMode,
  libraryOrder: string[],
): Game[] {
  const q = query.trim().toLowerCase();
  let list = games.filter((g) => {
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
  return [...list].sort((a, b) => {
    if (sortBy === "favorites") {
      if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
      return a.name.localeCompare(b.name);
    }
    if (sortBy === "missing") {
      if (a.missing !== b.missing) return a.missing ? -1 : 1;
      return a.name.localeCompare(b.name);
    }
    if (sortBy === "playtime") return b.playtimeMinutes - a.playtimeMinutes || a.name.localeCompare(b.name);
    if (sortBy === "recent") {
      const at = a.lastPlayedAt ? Date.parse(a.lastPlayedAt) : 0;
      const bt = b.lastPlayedAt ? Date.parse(b.lastPlayedAt) : 0;
      return bt - at || a.name.localeCompare(b.name);
    }
    if (sortBy === "added") {
      return Date.parse(b.dateAdded) - Date.parse(a.dateAdded) || a.name.localeCompare(b.name);
    }
    if (sortBy === "nameDesc") return b.name.localeCompare(a.name);
    if (sortBy === "custom") {
      const ai = libraryOrder.indexOf(gameOrderKey(a.id));
      const bi = libraryOrder.indexOf(gameOrderKey(b.id));
      return (ai < 0 ? 99999 : ai) - (bi < 0 ? 99999 : bi) || a.name.localeCompare(b.name);
    }
    return a.name.localeCompare(b.name);
  });
}

export function BigPicture({
  games,
  groups,
  coverMap,
  libraryOrder,
  reduceMotion,
  onLaunch,
  onToggleFavorite,
  onOpenFolder,
  onOpenSaveFolder,
  onExit,
  onToast,
}: {
  games: Game[];
  groups: GameGroup[];
  coverMap: Record<string, string>;
  libraryOrder: string[];
  reduceMotion: boolean;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onOpenFolder: (game: Game) => void;
  onOpenSaveFolder: (game: Game) => void;
  onExit: () => void;
  onToast: (text: string, error?: boolean) => void;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const [booting, setBooting] = useState(true);
  const [bootPct, setBootPct] = useState(0);
  const [bootLabel, setBootLabel] = useState("Starting IntelGen…");
  const [screen, setScreen] = useState<Screen>("library");
  const [zone, setZone] = useState<Zone>("stage");
  const [cursor, setCursor] = useState(0);
  const [listFocus, setListFocus] = useState("place:open");
  const [browseOpen, setBrowseOpen] = useState(false);
  const [browseFocus, setBrowseFocus] = useState("browse:search");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [fullGame, setFullGame] = useState<Game | null>(null);
  const [gameTab, setGameTab] = useState<GameTab>("overview");
  const [gameFocus, setGameFocus] = useState("gact:play");
  const [confirm, setConfirm] = useState<ConfirmKind | null>(null);
  const [confirmFocus, setConfirmFocus] = useState("confirm:no");
  const [muted, setMuted] = useState(isBpMuted);
  const [now, setNow] = useState(() => new Date());
  const [query, setQuery] = useState("");
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter>("all");
  const [sortBy, setSortBy] = useState<SortMode>("recent");
  const [openGroupId, setOpenGroupId] = useState<string | null>(null);
  const [places, setPlaces] = useState<ExplorerPlace[]>([]);
  const [listing, setListing] = useState<ExplorerListing | null>(null);
  const [explorerPath, setExplorerPath] = useState<string | null>(null);
  const [explorerBusy, setExplorerBusy] = useState(false);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [overview, setOverview] = useState<LibraryOverview | null>(null);
  const [overviewLoading, setOverviewLoading] = useState(false);

  const screenRef = useRef(screen);
  const zoneRef = useRef(zone);
  const cursorRef = useRef(cursor);
  const bootingRef = useRef(booting);
  const selectedIdRef = useRef(selectedId);
  const confirmRef = useRef(confirm);
  const browseOpenRef = useRef(browseOpen);
  const listingRef = useRef(listing);
  const gamesRef = useRef(games);
  const openGroupRef = useRef(openGroupId);
  const queryRef = useRef(query);
  const listFocusRef = useRef(listFocus);
  const browseFocusRef = useRef(browseFocus);
  const gameFocusRef = useRef(gameFocus);
  const confirmFocusRef = useRef(confirmFocus);
  const stripLenRef = useRef(0);
  const listKeysRef = useRef<string[]>([]);
  const browseKeysRef = useRef<string[]>([]);
  screenRef.current = screen;
  zoneRef.current = zone;
  cursorRef.current = cursor;
  bootingRef.current = booting;
  selectedIdRef.current = selectedId;
  confirmRef.current = confirm;
  browseOpenRef.current = browseOpen;
  listingRef.current = listing;
  gamesRef.current = games;
  openGroupRef.current = openGroupId;
  queryRef.current = query;
  listFocusRef.current = listFocus;
  browseFocusRef.current = browseFocus;
  gameFocusRef.current = gameFocus;
  confirmFocusRef.current = confirmFocus;

  const selected = games.find((g) => g.id === selectedId) ?? null;
  const { time } = clockParts(now);
  const bootDur = reduceMotion ? BOOT_MS_FAST : BOOT_MS;

  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const apply = () => {
      const s = Math.min(el.clientWidth / 1920, el.clientHeight / 1080);
      el.style.setProperty("--bp-scale", String(Math.max(0.72, Math.min(s, 1.85))));
    };
    apply();
    const ro = new ResizeObserver(apply);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 10000);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    playBoot();
    const started = performance.now();
    let raf = 0;
    const tick = (t: number) => {
      const p = Math.min(1, (t - started) / bootDur);
      setBootPct(Math.round(p * 100));
      if (p < 0.3) setBootLabel("Starting IntelGen…");
      else if (p < 0.62) setBootLabel("Loading library…");
      else if (p < 0.88) setBootLabel("Preparing stage…");
      else setBootLabel("Ready");
      if (p < 1) raf = requestAnimationFrame(tick);
      else {
        playReady();
        setBooting(false);
      }
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [bootDur]);

  const skipBoot = useCallback(() => {
    if (!bootingRef.current) return;
    setBootPct(100);
    setBooting(false);
    playReady();
  }, []);

  const filtered = useMemo(
    () => filterAndSort(games, query, libraryFilter, sortBy, libraryOrder),
    [games, query, libraryFilter, sortBy, libraryOrder],
  );

  const visibleGroups = useMemo(
    () =>
      groups
        .map((group) => ({
          group,
          members: group.gameIds
            .map((id) => filtered.find((g) => g.id === id))
            .filter((g): g is Game => Boolean(g)),
        }))
        .filter((item) => item.members.length > 0),
    [groups, filtered],
  );

  const openGroup = visibleGroups.find((g) => g.group.id === openGroupId) ?? null;

  const strip: StripItem[] = useMemo(() => {
    if (openGroup) return openGroup.members.map((game) => ({ kind: "game", id: game.id, game }));
    const items: StripItem[] = visibleGroups.map((item) => ({
      kind: "folder",
      id: item.group.id,
      group: item.group,
      members: item.members,
    }));
    const grouped = new Set(visibleGroups.flatMap((g) => g.members.map((m) => m.id)));
    for (const game of filtered) {
      if (!grouped.has(game.id)) items.push({ kind: "game", id: game.id, game });
    }
    if (items.length === 0) {
      for (const game of filtered) items.push({ kind: "game", id: game.id, game });
    }
    return items;
  }, [openGroup, visibleGroups, filtered]);

  stripLenRef.current = strip.length;
  const center = strip[Math.min(cursor, Math.max(0, strip.length - 1))] ?? null;
  const centerGame = center?.kind === "game" ? center.game : center?.members[0] ?? null;
  const bgSrc = centerGame ? coverSrc(centerGame, coverMap[centerGame.id]) : null;

  useEffect(() => {
    setCursor((c) => Math.max(0, Math.min(c, Math.max(0, strip.length - 1))));
  }, [strip.length]);

  useEffect(() => {
    if (openGroupId && !openGroup) setOpenGroupId(null);
  }, [openGroupId, openGroup]);

  useEffect(() => {
    if (screen !== "explorer") return;
    let cancelled = false;
    invoke<ExplorerPlace[]>("explorer_places")
      .then((p) => {
        if (!cancelled) setPlaces(p);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [screen]);

  useEffect(() => {
    if (screen !== "explorer") return;
    let cancelled = false;
    setExplorerBusy(true);
    invoke<ExplorerListing>("list_explorer", { path: explorerPath })
      .then((list) => {
        if (!cancelled) setListing(list);
      })
      .catch((e) => {
        if (!cancelled) onToast(String(e), true);
      })
      .finally(() => {
        if (!cancelled) setExplorerBusy(false);
      });
    return () => {
      cancelled = true;
    };
  }, [screen, explorerPath, onToast]);

  useEffect(() => {
    if (screen !== "system") return;
    let cancelled = false;
    const load = () => {
      invoke<SystemInfo>("system_info")
        .then((next) => {
          if (!cancelled) setInfo(next);
        })
        .catch(() => undefined);
    };
    load();
    const id = window.setInterval(load, 2500);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [screen]);

  useEffect(() => {
    if (screen !== "stats") return;
    let cancelled = false;
    setOverviewLoading(true);
    invoke<LibraryOverview>("library_overview")
      .then((data) => {
        if (!cancelled) setOverview(data);
      })
      .catch((e) => {
        if (!cancelled) onToast(String(e), true);
      })
      .finally(() => {
        if (!cancelled) setOverviewLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [screen, onToast]);

  useEffect(() => {
    if (!selectedId) {
      setFullGame(null);
      setGameTab("overview");
      setGameFocus("gact:play");
      return;
    }
    let cancelled = false;
    invoke<Game>("get_game", { id: selectedId })
      .then((loaded) => {
        if (cancelled) return;
        setFullGame(loaded);
        if (loaded.developer && loaded.description) return;
        return invoke<Game>("fetch_game_metadata", { id: selectedId }).then((updated) => {
          if (!cancelled) setFullGame(updated);
        });
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [selectedId]);

  const listKeys = useMemo(() => {
    if (screen === "explorer") {
      const keys = ["place:open", ...places.map((p) => `place:${p.id}`)];
      if (listing?.path || listing?.parent) keys.push("file:up");
      if (listing) for (const e of listing.entries) keys.push(`file:${e.path}`);
      return keys;
    }
    if (screen === "stats") {
      const keys = ["stat:week", "stat:streak", "stat:most", "stat:all"];
      for (const g of overview?.yearInReview.topGames ?? []) keys.push(`statgame:${g.gameId}`);
      return keys;
    }
    if (screen === "system") {
      return ["power:leave", "power:sleep", "power:lock", "power:restart", "power:shutdown"];
    }
    return [];
  }, [screen, places, listing, overview]);
  listKeysRef.current = listKeys;

  const browseKeys = useMemo(
    () => ["browse:search", ...BP_FILTERS.map((f) => `filter:${f.id}`), ...BP_SORTS.map((s) => `sort:${s.id}`)],
    [],
  );
  browseKeysRef.current = browseKeys;

  useEffect(() => {
    if (screen === "library") setZone("stage");
    else {
      setZone("list");
      setListFocus(listKeys[0] ?? "place:open");
    }
  }, [screen, listKeys]);

  const goScreen = useCallback((next: Screen) => {
    setScreen(next);
    setSelectedId(null);
    setBrowseOpen(false);
    setConfirm(null);
  }, []);

  const cycleScreen = useCallback((dir: 1 | -1) => {
    const i = TABS.findIndex((t) => t.id === screenRef.current);
    const next = TABS[(i + dir + TABS.length) % TABS.length];
    playMove();
    goScreen(next.id);
  }, [goScreen]);

  const toggleMute = useCallback(() => {
    const next = !isBpMuted();
    setBpMuted(next);
    setMuted(next);
    if (!next) playConfirm();
  }, []);

  const playCentered = useCallback(() => {
    const item = strip[cursorRef.current];
    if (!item) return;
    if (item.kind === "folder") {
      playConfirm();
      setOpenGroupId(item.group.id);
      setCursor(0);
      return;
    }
    if (item.game.missing) {
      playError();
      onToast("This game looks missing from disk", true);
      return;
    }
    playConfirm();
    onLaunch(item.game);
  }, [onLaunch, onToast, strip]);

  const openCenteredInfo = useCallback(() => {
    const item = strip[cursorRef.current];
    if (!item) return;
    if (item.kind === "folder") {
      playConfirm();
      setOpenGroupId(item.group.id);
      setCursor(0);
      return;
    }
    playConfirm();
    setSelectedId(item.game.id);
  }, [strip]);

  const activateList = useCallback(
    (key: string) => {
      if (key === "place:open") {
        playConfirm();
        invoke("open_explorer", { path: listingRef.current?.path ?? explorerPath })
          .then(() => onToast("Opened File Explorer"))
          .catch((e) => {
            playError();
            onToast(String(e), true);
          });
        return;
      }
      if (key.startsWith("place:")) {
        const place = places.find((p) => p.id === key.slice(6));
        if (!place) return;
        playConfirm();
        setExplorerPath(place.path);
        return;
      }
      if (key === "file:up") {
        playBack();
        setExplorerPath(listingRef.current?.parent ?? null);
        return;
      }
      if (key.startsWith("file:")) {
        const entry = listingRef.current?.entries.find((e) => e.path === key.slice(5));
        if (!entry) return;
        if (entry.isDir) {
          playConfirm();
          setExplorerPath(entry.path);
        } else {
          playConfirm();
          invoke("open_path", { path: entry.path }).catch((e) => {
            playError();
            onToast(String(e), true);
          });
        }
        return;
      }
      if (key === "stat:most" || key.startsWith("statgame:")) {
        const id = key === "stat:most" ? overview?.mostPlayed?.gameId : key.slice(9);
        const game = gamesRef.current.find((g) => g.id === id);
        if (game) {
          playConfirm();
          setSelectedId(game.id);
        }
        return;
      }
      if (key.startsWith("stat:")) return;
      if (key === "power:leave") {
        playConfirm();
        setConfirm("exit");
        setConfirmFocus("confirm:yes");
        return;
      }
      if (key === "power:sleep") {
        playPower();
        invoke("system_power", { action: "sleep" }).catch((e) => {
          playError();
          onToast(String(e), true);
        });
        return;
      }
      if (key === "power:lock") {
        playPower();
        invoke("system_power", { action: "lock" }).catch((e) => {
          playError();
          onToast(String(e), true);
        });
        return;
      }
      if (key === "power:restart") {
        playConfirm();
        setConfirm("restart");
        setConfirmFocus("confirm:no");
        return;
      }
      if (key === "power:shutdown") {
        playConfirm();
        setConfirm("shutdown");
        setConfirmFocus("confirm:no");
      }
    },
    [explorerPath, onToast, overview, places],
  );

  const activateGame = useCallback(
    (key: string) => {
      const game = gamesRef.current.find((g) => g.id === selectedIdRef.current);
      if (!game) return;
      if (key === "gtab:overview") {
        playMove();
        setGameTab("overview");
        return;
      }
      if (key === "gtab:sessions") {
        playMove();
        setGameTab("sessions");
        return;
      }
      if (key === "gact:play") {
        if (game.missing) {
          playError();
          onToast("This game looks missing from disk", true);
          return;
        }
        playConfirm();
        onLaunch(game);
        return;
      }
      if (key === "gact:fav") {
        playConfirm();
        onToggleFavorite(game);
        return;
      }
      if (key === "gact:folder") {
        playConfirm();
        onOpenFolder(game);
        return;
      }
      if (key === "gact:save") {
        playConfirm();
        onOpenSaveFolder(game);
        return;
      }
      if (key === "gact:back") {
        playBack();
        setSelectedId(null);
      }
    },
    [onLaunch, onOpenFolder, onOpenSaveFolder, onToast, onToggleFavorite],
  );

  const goBack = useCallback(() => {
    if (bootingRef.current) {
      skipBoot();
      return;
    }
    if (confirmRef.current) {
      playBack();
      setConfirm(null);
      return;
    }
    if (selectedIdRef.current) {
      playBack();
      setSelectedId(null);
      return;
    }
    if (browseOpenRef.current) {
      playBack();
      setBrowseOpen(false);
      return;
    }
    if (screenRef.current === "explorer" && listingRef.current?.path) {
      playBack();
      setExplorerPath(listingRef.current.parent);
      return;
    }
    if (openGroupRef.current) {
      playBack();
      setOpenGroupId(null);
      setCursor(0);
      return;
    }
    if (queryRef.current) {
      playBack();
      setQuery("");
      return;
    }
    if (screenRef.current !== "library") {
      playBack();
      goScreen("library");
      return;
    }
    if (zoneRef.current === "tabs") {
      playMove();
      setZone("stage");
      return;
    }
    playBack();
    setConfirm("exit");
    setConfirmFocus("confirm:no");
  }, [goScreen, skipBoot]);

  const moveList = (dx: number, dy: number) => {
    const keys = listKeysRef.current;
    if (!keys.length) return;
    let idx = keys.indexOf(listFocusRef.current);
    if (idx < 0) idx = 0;
    const cols = screenRef.current === "system" ? 5 : screenRef.current === "stats" ? 4 : 1;
    let next = idx;
    if (dx !== 0) next = Math.max(0, Math.min(keys.length - 1, idx + dx));
    else next = Math.max(0, Math.min(keys.length - 1, idx + dy * cols));
    if (keys[next] !== listFocusRef.current) playMove();
    setListFocus(keys[next]);
  };

  const handlePad = useCallback(
    (action: PadAction) => {
      if (bootingRef.current) {
        if (action === "confirm" || action === "back") skipBoot();
        return;
      }
      if (confirmRef.current) {
        if (action === "left" || action === "right") {
          playMove();
          setConfirmFocus((cur) => (cur === "confirm:no" ? "confirm:yes" : "confirm:no"));
        } else if (action === "confirm") {
          const kind = confirmRef.current;
          if (confirmFocusRef.current === "confirm:yes") {
            if (kind === "exit") {
              playBack();
              onExit();
              return;
            }
            playPower();
            invoke("system_power", { action: kind }).catch((e) => {
              playError();
              onToast(String(e), true);
            });
          } else playBack();
          setConfirm(null);
        } else if (action === "back") {
          playBack();
          setConfirm(null);
        }
        return;
      }
      if (action === "lb") {
        setSelectedId(null);
        setBrowseOpen(false);
        cycleScreen(-1);
        return;
      }
      if (action === "rb") {
        setSelectedId(null);
        setBrowseOpen(false);
        cycleScreen(1);
        return;
      }
      if (selectedIdRef.current) {
        const keys = [
          "gact:play",
          "gact:fav",
          "gact:folder",
          "gact:save",
          "gact:back",
          "gtab:overview",
          "gtab:sessions",
        ];
        if (action === "left" || action === "right") {
          const acts = keys.filter((k) => k.startsWith("gact:"));
          let i = acts.indexOf(gameFocusRef.current);
          if (i < 0) i = 0;
          const n = Math.max(0, Math.min(acts.length - 1, i + (action === "right" ? 1 : -1)));
          if (acts[n] !== gameFocusRef.current) playMove();
          setGameFocus(acts[n]);
        } else if (action === "up") {
          playMove();
          setGameFocus("gtab:overview");
        } else if (action === "down") {
          playMove();
          setGameFocus("gact:play");
        } else if (action === "confirm") activateGame(gameFocusRef.current);
        else if (action === "back") goBack();
        else if (action === "favorite") activateGame("gact:fav");
        else if (action === "alt") activateGame("gact:play");
        return;
      }
      if (browseOpenRef.current) {
        if (isTypingTarget(document.activeElement) && (action === "left" || action === "right")) return;
        const keys = browseKeysRef.current;
        let idx = keys.indexOf(browseFocusRef.current);
        if (idx < 0) idx = 0;
        if (action === "left" || action === "right" || action === "up" || action === "down") {
          const dy = action === "down" ? 1 : action === "up" ? -1 : 0;
          const dx = action === "right" ? 1 : action === "left" ? -1 : 0;
          const next = Math.max(0, Math.min(keys.length - 1, idx + dx + dy));
          if (keys[next] !== browseFocusRef.current) playMove();
          setBrowseFocus(keys[next]);
          if (keys[next] === "browse:search") searchRef.current?.focus();
          else searchRef.current?.blur();
        } else if (action === "confirm") {
          const key = browseFocusRef.current;
          if (key.startsWith("filter:")) {
            playConfirm();
            setLibraryFilter(key.slice(7) as LibraryFilter);
          } else if (key.startsWith("sort:")) {
            playConfirm();
            setSortBy(key.slice(5) as SortMode);
          } else searchRef.current?.focus();
        } else if (action === "back" || action === "alt") {
          playBack();
          setBrowseOpen(false);
        }
        return;
      }
      if (action === "menu") {
        playConfirm();
        goScreen("system");
        return;
      }
      if (action === "select" || action === "alt") {
        if (screenRef.current === "library") {
          playConfirm();
          setBrowseOpen(true);
          setBrowseFocus("browse:search");
          window.setTimeout(() => searchRef.current?.focus(), 0);
        } else if (action === "alt" && screenRef.current === "explorer") {
          playConfirm();
          invoke("open_explorer", { path: listingRef.current?.path ?? explorerPath }).catch((e) => {
            playError();
            onToast(String(e), true);
          });
        }
        return;
      }
      if (action === "back") {
        goBack();
        return;
      }

      if (zoneRef.current === "tabs") {
        if (action === "up") cycleScreen(-1);
        else if (action === "down") cycleScreen(1);
        else if (action === "right" || action === "confirm") {
          playMove();
          setZone(screenRef.current === "library" ? "stage" : "list");
        }
        return;
      }

      if (screenRef.current === "library") {
        if (action === "left") {
          if (cursorRef.current > 0) {
            playMove();
            setCursor((c) => c - 1);
          } else {
            playMove();
            setZone("tabs");
          }
        } else if (action === "right") {
          if (cursorRef.current < stripLenRef.current - 1) {
            playMove();
            setCursor((c) => c + 1);
          }
        } else if (action === "up") {
          playMove();
          setZone("tabs");
        } else if (action === "confirm") playCentered();
        else if (action === "favorite") openCenteredInfo();
        return;
      }

      if (action === "up") {
        const keys = listKeysRef.current;
        const idx = keys.indexOf(listFocusRef.current);
        if (idx <= 0) {
          playMove();
          setZone("tabs");
          return;
        }
        moveList(0, -1);
      } else if (action === "down") moveList(0, 1);
      else if (action === "left") {
        const keys = listKeysRef.current;
        const idx = keys.indexOf(listFocusRef.current);
        if (idx <= 0 || screenRef.current === "explorer") {
          playMove();
          setZone("tabs");
        } else moveList(-1, 0);
      } else if (action === "right") moveList(1, 0);
      else if (action === "confirm") activateList(listFocusRef.current);
      else if (action === "favorite") {
        const key = listFocusRef.current;
        const id = key.startsWith("statgame:") ? key.slice(9) : null;
        const game = gamesRef.current.find((g) => g.id === id);
        if (game) {
          playConfirm();
          onToggleFavorite(game);
        }
      }
    },
    [
      activateGame,
      activateList,
      cycleScreen,
      explorerPath,
      goBack,
      goScreen,
      onExit,
      onToast,
      onToggleFavorite,
      openCenteredInfo,
      playCentered,
      skipBoot,
    ],
  );

  const padOn = useGamepad(handlePad, true);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (isTypingTarget(e.target)) {
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          (e.target as HTMLElement).blur();
          if (browseOpenRef.current) setBrowseOpen(false);
        }
        return;
      }
      const map: Record<string, PadAction | undefined> = {
        Escape: "back",
        Enter: "confirm",
        ArrowLeft: "left",
        ArrowRight: "right",
        ArrowUp: "up",
        ArrowDown: "down",
        q: "lb",
        Q: "lb",
        e: "rb",
        E: "rb",
        y: "alt",
        Y: "alt",
      };
      if (e.key === "/" || ((e.ctrlKey || e.metaKey) && e.key === "k")) {
        e.preventDefault();
        e.stopPropagation();
        goScreen("library");
        setBrowseOpen(true);
        setBrowseFocus("browse:search");
        window.setTimeout(() => searchRef.current?.focus(), 0);
        return;
      }
      if (e.key === "f" || e.key === "F" || e.key === "x" || e.key === "X") {
        e.preventDefault();
        e.stopPropagation();
        handlePad("favorite");
        return;
      }
      if (e.key === "m" || e.key === "M") {
        e.preventDefault();
        toggleMute();
        return;
      }
      const action = map[e.key];
      if (!action) return;
      e.preventDefault();
      e.stopPropagation();
      handlePad(action);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [goScreen, handlePad, toggleMute]);

  const ramPct =
    info && info.ramTotalBytes > 0
      ? Math.min(100, (info.ramUsedBytes / info.ramTotalBytes) * 100)
      : 0;

  const gameKeys = (() => {
    const keys = ["gact:play", "gact:fav"];
    if (selected?.installPath || fullGame?.installPath) keys.push("gact:folder");
    if (fullGame?.saveFolder) keys.push("gact:save");
    keys.push("gact:back");
    return keys;
  })();

  return (
    <div
      ref={rootRef}
      className={`bp-root${reduceMotion ? " reduce-motion" : ""}${booting ? " bp-booting" : ""}`}
      role="application"
      aria-label="Big Picture"
    >
      {bgSrc && screen === "library" && !selected && !browseOpen && (
        <div className="bp-stage-bg" style={{ backgroundImage: `url("${bgSrc.replace(/"/g, "")}")` }} />
      )}
      <div className="bp-stage-veil" />

      {booting && (
        <div className="bp-boot" onClick={skipBoot}>
          <img src="/intelgen-icon.png" alt="" className="bp-boot-mark" />
          <h1 className="bp-boot-title">IntelGen</h1>
          <p className="bp-boot-sub">Big Picture</p>
          <div className="bp-boot-bar" role="progressbar" aria-valuenow={bootPct}>
            <span style={{ width: `${bootPct}%` }} />
          </div>
          <p className="bp-boot-label">{bootLabel}</p>
          <p className="bp-boot-skip">
            <Glyph face="A" /> Skip
          </p>
        </div>
      )}

      <header className="bp-hud">
        <div className="bp-hud-brand">
          <img src="/intelgen-icon.png" alt="" className="bp-hud-mark" />
          <span className="bp-hud-name">IntelGen</span>
        </div>
        <div className="bp-hud-clock">{time}</div>
      </header>

      <div className="bp-body">
        <nav className={`bp-rail${zone === "tabs" ? " is-hot" : ""}`} aria-label="Sections">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              className={`bp-rail-btn${screen === t.id ? " is-active" : ""}`}
              onClick={() => {
                playConfirm();
                goScreen(t.id);
              }}
            >
              <span className="bp-rail-icon" aria-hidden>
                {t.icon}
              </span>
              <span className="bp-rail-label">{t.label}</span>
            </button>
          ))}
        </nav>
        <div className="bp-main">
      {!selected && screen === "library" && (
        <div className="bp-stage">
              {strip.length === 0 ? (
                <p className="bp-empty">
                  No games in this view. <Glyph face="Y" label="Browse" />
                </p>
              ) : (
                <>
                  <p className="bp-row-label">{openGroup ? openGroup.group.name : "Games"}</p>
                  <div className="bp-flow" aria-label="Game carousel">
                    {strip.map((item, i) => {
                      const off = i - cursor;
                      if (Math.abs(off) > FLOW_WINDOW) return null;
                      return (
                        <button
                          key={item.id}
                          type="button"
                          className={`bp-flow-card${off === 0 ? " is-center" : ""}`}
                          style={{
                            zIndex: 20 - Math.abs(off),
                            transform: `translateX(${off * 12.4}vw) scale(${off === 0 ? 1.12 : 0.86 - Math.abs(off) * 0.04})`,
                            opacity: Math.max(0.32, 1 - Math.abs(off) * 0.16),
                          }}
                      onClick={() => {
                        if (i === cursor) playCentered();
                        else {
                          playMove();
                          setCursor(i);
                        }
                      }}
                    >
                      <div className="cover">
                        {item.kind === "folder" ? (
                          <div className="stack-layers">
                            {item.members.slice(0, 3).map((g, n) => (
                              <div key={g.id} className={`stack-card stack-card-${n}`} style={{ zIndex: 3 - n }}>
                                <CoverImg game={g} override={coverMap[g.id]} loading="eager" allowRemote={false} />
                              </div>
                            ))}
                          </div>
                        ) : (
                          <CoverImg
                            game={item.game}
                            override={coverMap[item.game.id]}
                            loading="eager"
                            allowRemote={false}
                          />
                        )}
                      </div>
                    </button>
                  );
                })}
              </div>
              {center && (
                <div className="bp-now">
                  <h2>{center.kind === "folder" ? center.group.name : center.game.name}</h2>
                  <p>
                    {center.kind === "folder"
                      ? `${center.members.length} games`
                      : `${STORE_LABELS[center.game.store as Store] ?? center.game.store}${
                          center.game.playtimeMinutes > 0 ? `  ·  ${formatPlaytime(center.game.playtimeMinutes)}` : ""
                        }`}
                  </p>
                  <button type="button" className="bp-play" onClick={playCentered}>
                    {center.kind === "folder" ? "Open folder" : "Play"}
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      )}

      {!selected && screen === "explorer" && (
        <ExplorerScreen
          places={places}
          listing={listing}
          busy={explorerBusy}
          focus={listFocus}
          onPlace={(id, path) => {
            if (id === "open") activateList("place:open");
            else {
              playConfirm();
              setExplorerPath(path);
            }
          }}
          onUp={() => activateList("file:up")}
          onEntry={(e) => activateList(`file:${e.path}`)}
        />
      )}
      {!selected && screen === "stats" && (
        <StatsScreen
          overview={overview}
          loading={overviewLoading}
          focus={listFocus}
          onOpenGame={(id) => {
            const g = games.find((x) => x.id === id);
            if (g) {
              playConfirm();
              setSelectedId(g.id);
            }
          }}
        />
      )}
      {!selected && screen === "system" && (
        <SystemScreen
          info={info}
          ramPct={ramPct}
          focus={listFocus}
          muted={muted}
          onMute={toggleMute}
          onPower={(action) => activateList(`power:${action}`)}
        />
      )}

          {selected && (
            <GameDataPage
              game={selected}
              full={fullGame}
              cover={coverMap[selected.id]}
              tab={gameTab}
              focus={gameFocus}
              actionKeys={gameKeys}
              onActivate={activateGame}
            />
          )}
        </div>
      </div>

      {browseOpen && (
        <div className="bp-browse" role="dialog" aria-label="Filter and search">
          <div className="bp-browse-panel">
            <h2>Browse</h2>
            <input
              ref={searchRef}
              className={browseFocus === "browse:search" ? "is-focus" : ""}
              data-bp-key="browse:search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onFocus={() => setBrowseFocus("browse:search")}
              placeholder="Search library"
            />
            <p className="bp-browse-label">Filter</p>
            <div className="bp-chips">
              {BP_FILTERS.map((f) => (
                <button
                  key={f.id}
                  type="button"
                  className={`bp-chip${libraryFilter === f.id ? " is-active" : ""}${browseFocus === `filter:${f.id}` ? " is-focus" : ""}`}
                  onClick={() => {
                    playConfirm();
                    setLibraryFilter(f.id);
                  }}
                >
                  {f.label}
                </button>
              ))}
            </div>
            <p className="bp-browse-label">Sort</p>
            <div className="bp-chips">
              {BP_SORTS.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  className={`bp-chip${sortBy === s.id ? " is-active" : ""}${browseFocus === `sort:${s.id}` ? " is-focus" : ""}`}
                  onClick={() => {
                    playConfirm();
                    setSortBy(s.id);
                  }}
                >
                  {s.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {confirm && (
        <ConfirmSheet
          kind={confirm}
          focus={confirmFocus}
          onNo={() => {
            playBack();
            setConfirm(null);
          }}
          onYes={() => {
            if (confirm === "exit") {
              playBack();
              onExit();
              return;
            }
            playPower();
            invoke("system_power", { action: confirm }).catch((e) => {
              playError();
              onToast(String(e), true);
            });
            setConfirm(null);
          }}
        />
      )}

      <footer className="bp-bar">
        <div className="bp-bar-prompts">
          {screen === "library" && !selected ? (
            <>
              <Glyph face="A" label="Play" />
              <Glyph face="X" label="Info" />
              <Glyph face="Y" label="Browse" />
              <Glyph face="B" label="Back" />
              <Glyph face="LB" />
              <Glyph face="RB" label="Sections" />
            </>
          ) : selected ? (
            <>
              <Glyph face="A" label="Select" />
              <Glyph face="X" label="Favorite" />
              <Glyph face="B" label="Back" />
              <Glyph face="LB" />
              <Glyph face="RB" label="Sections" />
            </>
          ) : (
            <>
              <Glyph face="A" label="Select" />
              <Glyph face="B" label="Back" />
              <Glyph face="LB" />
              <Glyph face="RB" label="Sections" />
            </>
          )}
        </div>
        <span className={`bp-bar-keys${padOn ? " is-dim" : ""}`}>
          {padOn ? "Keyboard fallback: arrows · Enter · Esc · Q/E" : "No controller · arrows · Enter · Esc · Q/E · Y browse"}
        </span>
      </footer>
    </div>
  );
}

function Glyph({ face, label }: { face: "A" | "B" | "X" | "Y" | "LB" | "RB"; label?: string }) {
  return (
    <span className="bp-glyph">
      <i className={`bp-face bp-face-${face.toLowerCase()}`}>{face}</i>
      {label ? <em>{label}</em> : null}
    </span>
  );
}

function ExplorerScreen({
  places,
  listing,
  busy,
  focus,
  onPlace,
  onUp,
  onEntry,
}: {
  places: ExplorerPlace[];
  listing: ExplorerListing | null;
  busy: boolean;
  focus: string;
  onPlace: (id: string, path: string | null) => void;
  onUp: () => void;
  onEntry: (entry: ExplorerEntry) => void;
}) {
  return (
    <section className="bp-panel">
      <h2 className="bp-panel-title">{listing?.label ?? "Files"}</h2>
      {listing?.path && <p className="bp-path">{listing.path}</p>}
      <div className="bp-places">
        <button
          type="button"
          className={`bp-place${focus === "place:open" ? " is-focus" : ""}`}
          onClick={() => onPlace("open", listing?.path ?? null)}
        >
          Open File Explorer
        </button>
        {places.map((p) => (
          <button
            key={p.id}
            type="button"
            className={`bp-place${focus === `place:${p.id}` ? " is-focus" : ""}`}
            onClick={() => onPlace(p.id, p.path)}
          >
            {p.name}
          </button>
        ))}
      </div>
      <div className={`bp-files${busy ? " is-busy" : ""}`}>
        {(listing?.path || listing?.parent) && (
          <button type="button" className={`bp-file${focus === "file:up" ? " is-focus" : ""}`} onClick={onUp}>
            <span className="bp-file-glyph">↑</span>
            <span className="bp-file-name">Up</span>
            <span className="bp-file-kind">Parent folder</span>
          </button>
        )}
        {(listing?.entries ?? []).map((e) => (
          <button
            key={e.path}
            type="button"
            className={`bp-file${focus === `file:${e.path}` ? " is-focus" : ""}`}
            onClick={() => onEntry(e)}
          >
            <span className="bp-file-glyph">{kindGlyph(e.kind)}</span>
            <span className="bp-file-name">{e.name}</span>
            <span className="bp-file-kind">{e.isDir ? "Folder" : e.kind}</span>
          </button>
        ))}
      </div>
    </section>
  );
}

function StatsScreen({
  overview,
  loading,
  focus,
  onOpenGame,
}: {
  overview: LibraryOverview | null;
  loading: boolean;
  focus: string;
  onOpenGame: (id: string) => void;
}) {
  const y = overview?.yearInReview;
  const monthly =
    y?.monthly.map((m) => ({
      label: m.day.slice(5),
      value: Math.round(m.minutes / 60),
    })) ?? [];
  return (
    <section className="bp-panel">
      <h2 className="bp-panel-title">Stats</h2>
      {loading || !overview ? (
        <p className="bp-empty">Crunching playtime…</p>
      ) : (
        <>
          <div className="stats-hero-grid">
            <div className={`stat-card${focus === "stat:week" ? " is-focus" : ""}`}>
              <span className="stat-label">This week</span>
              <span className="stat-value">{formatPlaytime(overview.minutesThisWeek)}</span>
            </div>
            <div className={`stat-card${focus === "stat:streak" ? " is-focus" : ""}`}>
              <span className="stat-label">Play streak</span>
              <span className="stat-value">
                {overview.streakDays > 0 ? `${overview.streakDays} days` : "—"}
              </span>
            </div>
            <button
              type="button"
              className={`stat-card stat-link${focus === "stat:most" ? " is-focus" : ""}`}
              onClick={() => overview.mostPlayed && onOpenGame(overview.mostPlayed.gameId)}
            >
              <span className="stat-label">Most played</span>
              <span className="stat-value">{overview.mostPlayed?.name ?? "—"}</span>
            </button>
            <div className={`stat-card${focus === "stat:all" ? " is-focus" : ""}`}>
              <span className="stat-label">All-time</span>
              <span className="stat-value">{formatPlaytime(overview.totalPlaytimeMinutes)}</span>
            </div>
          </div>
          {y && (
            <div className="charts-row">
              <BarChart title="Hours by month" points={monthly} formatValue={(v) => `${v}h`} />
              <div className="chart-card">
                <h3>Top games</h3>
                <ol className="top-games">
                  {y.topGames.map((g) => (
                    <li key={g.gameId}>
                      <button
                        type="button"
                        className={focus === `statgame:${g.gameId}` ? "is-focus" : ""}
                        onClick={() => onOpenGame(g.gameId)}
                      >
                        {g.name}
                      </button>
                      <span>{formatPlaytime(g.minutes)}</span>
                    </li>
                  ))}
                </ol>
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}

function SystemScreen({
  info,
  ramPct,
  focus,
  muted,
  onMute,
  onPower,
}: {
  info: SystemInfo | null;
  ramPct: number;
  focus: string;
  muted: boolean;
  onMute: () => void;
  onPower: (action: "leave" | "sleep" | "lock" | "restart" | "shutdown") => void;
}) {
  const powers: { id: "leave" | "sleep" | "lock" | "restart" | "shutdown"; label: string; sub: string }[] = [
    { id: "leave", label: "Desktop", sub: "Leave Big Picture" },
    { id: "sleep", label: "Sleep", sub: "Pause the PC" },
    { id: "lock", label: "Lock", sub: "Lock Windows" },
    { id: "restart", label: "Restart", sub: "Reboot" },
    { id: "shutdown", label: "Shut down", sub: "Power off" },
  ];
  return (
    <section className="bp-panel">
      <h2 className="bp-panel-title">System</h2>
      <div className="bp-power-row">
        {powers.map((p) => (
          <button
            key={p.id}
            type="button"
            className={`bp-power${focus === `power:${p.id}` ? " is-focus" : ""}${p.id === "shutdown" ? " is-danger" : ""}`}
            onClick={() => onPower(p.id)}
          >
            <span className="bp-power-label">{p.label}</span>
            <span className="bp-power-sub">{p.sub}</span>
          </button>
        ))}
      </div>
      <button type="button" className="bp-place" onClick={onMute}>
        {muted ? "Sound off" : "Sound on"}
      </button>
      <dl className="sys-facts bp-facts">
        <div className="sys-row">
          <dt>PC</dt>
          <dd>{info?.hostname || "—"}</dd>
        </div>
        <div className="sys-row">
          <dt>OS</dt>
          <dd>{[info?.os, info?.osVersion].filter(Boolean).join(" ") || "—"}</dd>
        </div>
        <div className="sys-row">
          <dt>Processor</dt>
          <dd>{info?.cpu || "—"}</dd>
        </div>
        <div className="sys-row">
          <dt>Graphics</dt>
          <dd>{info?.gpu || "—"}</dd>
        </div>
        <div className="sys-row">
          <dt>Memory</dt>
          <dd>
            {info ? `${formatBytes(info.ramTotalBytes)}  ·  ${formatBytes(info.ramUsedBytes)} in use` : "—"}
            <div className="ram-meter" aria-hidden>
              <div className="ram-meter-fill" style={{ width: `${ramPct}%` }} />
            </div>
          </dd>
        </div>
      </dl>
    </section>
  );
}

function GameDataPage({
  game,
  full,
  cover,
  tab,
  focus,
  actionKeys,
  onActivate,
}: {
  game: Game;
  full: Game | null;
  cover?: string;
  tab: GameTab;
  focus: string;
  actionKeys: string[];
  onActivate: (key: string) => void;
}) {
  const [stats, setStats] = useState<GameStats | null>(null);
  const view =
    full && full.id === game.id ? { ...full, ...game, description: full.description ?? game.description } : game;
  const genres = view.genres?.length
    ? view.genres
    : view.genre
      ? view.genre.split(",").map((s) => s.trim())
      : [];
  const art = coverSourceLabel(view);

  useEffect(() => {
    let cancelled = false;
    invoke<GameStats>("get_game_stats", { id: game.id })
      .then((s) => {
        if (!cancelled) setStats(s);
      })
      .catch(() => {
        if (!cancelled) setStats(null);
      });
    return () => {
      cancelled = true;
    };
  }, [game.id, game.playtimeMinutes]);

  const dailyPoints =
    stats?.dailyPlaytime.map((d) => ({ label: d.day.slice(5), value: d.minutes })) ?? [];

  return (
    <div className="bp-gamepage">
      <div className="bp-gamepage-hero">
        <div className="bp-sheet-art">
          <div className="cover">
            <CoverImg game={game} override={cover} loading="eager" allowRemote={false} />
          </div>
          {art ? <p className="cover-source">Art from {art}</p> : null}
        </div>
        <div className="bp-sheet-body">
          <h2>{game.name}</h2>
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
          </dl>
          <div className="bp-sheet-actions">
            {actionKeys.map((key) => (
              <button
                key={key}
                type="button"
                className={`btn${key === "gact:play" ? " btn-primary" : ""}${focus === key ? " is-focus" : ""}`}
                disabled={key === "gact:play" && game.missing}
                onClick={() => onActivate(key)}
              >
                {key === "gact:play"
                  ? "Play"
                  : key === "gact:fav"
                    ? game.favorite
                      ? "Favorited"
                      : "Favorite"
                    : key === "gact:folder"
                      ? "Install folder"
                      : key === "gact:save"
                        ? "Save folder"
                        : "Back"}
              </button>
            ))}
          </div>
        </div>
      </div>
      <div className="detail-tabs">
        <button
          type="button"
          className={`detail-tab${tab === "overview" ? " active" : ""}${focus === "gtab:overview" ? " is-focus" : ""}`}
          onClick={() => onActivate("gtab:overview")}
        >
          Overview
        </button>
        <button
          type="button"
          className={`detail-tab${tab === "sessions" ? " active" : ""}${focus === "gtab:sessions" ? " is-focus" : ""}`}
          onClick={() => onActivate("gtab:sessions")}
        >
          Sessions
        </button>
      </div>
      {tab === "overview" && (
        <div className="detail-tab-body">
          {view.description ? <p className="game-blurb">{view.description}</p> : null}
          <BarChart title="Playtime (last 14 days)" points={dailyPoints} formatValue={(v) => `${v}m`} />
        </div>
      )}
      {tab === "sessions" && (
        <div className="sessions-panel">
          {!stats || stats.sessions.length === 0 ? (
            <p className="chart-empty">No sessions yet.</p>
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
    </div>
  );
}

function ConfirmSheet({
  kind,
  focus,
  onNo,
  onYes,
}: {
  kind: ConfirmKind;
  focus: string;
  onNo: () => void;
  onYes: () => void;
}) {
  const copy =
    kind === "exit"
      ? { title: "Leave Big Picture?", body: "Return to the desktop launcher." }
      : kind === "restart"
        ? { title: "Restart this PC?", body: "Unsaved work may be lost." }
        : { title: "Shut down this PC?", body: "Unsaved work may be lost." };
  return (
    <div className="bp-sheet" onClick={onNo}>
      <div className="bp-confirm" onClick={(e) => e.stopPropagation()}>
        <h2>{copy.title}</h2>
        <p>{copy.body}</p>
        <div className="bp-sheet-actions">
          <button type="button" className={`btn${focus === "confirm:no" ? " is-focus" : ""}`} onClick={onNo}>
            Cancel
          </button>
          <button
            type="button"
            className={`btn ${kind === "exit" ? "btn-primary" : "bp-btn-danger"}${focus === "confirm:yes" ? " is-focus" : ""}`}
            onClick={onYes}
          >
            {kind === "exit" ? "Leave" : kind === "restart" ? "Restart" : "Shut down"}
          </button>
        </div>
      </div>
    </div>
  );
}
