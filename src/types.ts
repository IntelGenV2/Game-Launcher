import { invoke } from "@tauri-apps/api/core";

export type Store =
  | "steam"
  | "epic"
  | "gog"
  | "xbox"
  | "battlenet"
  | "ubisoft"
  | "ea"
  | "roblox"
  | "wargaming"
  | "riot"
  | "rockstar"
  | "amazon"
  | "itch"
  | "humble"
  | "manual";

export type ThemeId = "emerald" | "amber" | "cyan" | "slate" | "crimson";

export type GridDensity = "cozy" | "normal" | "compact";
export type CoverCorners = "soft" | "round" | "sharp";
export type CoverShape = "portrait" | "square";

export type LibraryFilter =
  | "all"
  | "favorites"
  | "never"
  | "missing"
  | "hidden"
  | "steam"
  | "epic"
  | "gog"
  | "xbox"
  | "ea"
  | "battlenet"
  | "ubisoft"
  | "wargaming"
  | "riot"
  | "rockstar"
  | "amazon"
  | "itch"
  | "humble"
  | "other";

export type MainView = "library" | "stats";

export interface Game {
  id: string;
  name: string;
  store: Store;
  launchTarget: string;
  installPath: string | null;
  coverUrl: string | null;
  coverPath: string | null;
  coverSource?: string | null;
  favorite: boolean;
  hidden: boolean;
  missing: boolean;
  playtimeMinutes: number;
  lastPlayedAt: string | null;
  dateAdded: string;
  steamAppId: string | null;
  genre: string | null;
  tags?: string[];
  notes: string | null;
  developer: string | null;
  publisher: string | null;
  releaseYear: number | null;
  description: string | null;
  genres?: string[];
  logoPath: string | null;
  launchArgs: string | null;
  workingDir: string | null;
  runAsAdmin: boolean;
  saveFolder: string | null;
}

/** Drop Wikipedia dumps and notes so the library list stays small. */
export function asLibraryGame(game: Game): Game {
  return {
    ...game,
    notes: null,
    description: null,
    launchArgs: null,
    workingDir: null,
    saveFolder: null,
    logoPath: null,
  };
}

export interface CoverChoiceGroup {
  gameId: string;
  name: string;
  currentPath: string | null;
  paths: string[];
}

export interface GameGroup {
  id: string;
  name: string;
  sortOrder: number;
  createdAt: string;
  gameIds: string[];
}

export interface AppSettings {
  steamGridDbApiKey: string | null;
  sortBy: string | null;
  theme: string | null;
  cardScale: number | null;
  libraryOrder: string | null;
  showTitles: boolean | null;
  showStoreLabels: boolean | null;
  gridDensity: string | null;
  coverCorners: string | null;
  coverShape: string | null;
  reduceMotion: boolean | null;
  startWithWindows: boolean | null;
  closeToTray: boolean | null;
  startInBackground: boolean | null;
  hideOnGameLaunch: boolean | null;
}

export interface LibraryStats {
  total: number;
  favorites: number;
  missing: number;
  totalPlaytimeMinutes: number;
}

export interface TopPlayedGame {
  gameId: string;
  name: string;
  minutes: number;
}

export interface YearInReview {
  year: number;
  totalMinutes: number;
  monthly: DailyPlaytime[];
  topGames: TopPlayedGame[];
}

export interface LibraryOverview {
  hoursThisWeek: number;
  minutesThisWeek: number;
  mostPlayed: TopPlayedGame | null;
  streakDays: number;
  yearInReview: YearInReview;
  totalPlaytimeMinutes: number;
  gamesPlayed: number;
}

export interface ScanStoreProgress {
  store: string;
  status: string;
  count: number;
  message: string | null;
}

export interface DuplicateGroup {
  key: string;
  games: Game[];
}

export function neverPlayed(game: Game): boolean {
  return (!game.playtimeMinutes || game.playtimeMinutes <= 0) && !game.lastPlayedAt;
}

export interface PlaySession {
  id: number;
  gameId: string;
  startedAt: string;
  endedAt: string | null;
  durationMinutes: number;
}

export interface DailyPlaytime {
  day: string;
  minutes: number;
}

export interface GameStats {
  gameId: string;
  totalPlaytimeMinutes: number;
  sessionCount: number;
  avgSessionMinutes: number;
  lastPlayedAt: string | null;
  firstPlayedAt: string | null;
  dailyPlaytime: DailyPlaytime[];
  sessions: PlaySession[];
}

export type SortMode =
  | "custom"
  | "name"
  | "nameDesc"
  | "recent"
  | "added"
  | "playtime"
  | "favorites"
  | "missing";

export const THEME_OPTIONS: { id: ThemeId; label: string }[] = [
  { id: "emerald", label: "Emerald" },
  { id: "amber", label: "Amber" },
  { id: "cyan", label: "Cyan" },
  { id: "slate", label: "Slate" },
  { id: "crimson", label: "Crimson" },
];

export const DENSITY_OPTIONS: { id: GridDensity; label: string }[] = [
  { id: "cozy", label: "Cozy" },
  { id: "normal", label: "Normal" },
  { id: "compact", label: "Compact" },
];

export const CORNER_OPTIONS: { id: CoverCorners; label: string }[] = [
  { id: "soft", label: "Soft" },
  { id: "round", label: "Round" },
  { id: "sharp", label: "Sharp" },
];

export const SHAPE_OPTIONS: { id: CoverShape; label: string }[] = [
  { id: "portrait", label: "Portrait" },
  { id: "square", label: "Square" },
];

export function isGridDensity(v: string | null | undefined): v is GridDensity {
  return v === "cozy" || v === "normal" || v === "compact";
}

export function isCoverCorners(v: string | null | undefined): v is CoverCorners {
  return v === "soft" || v === "round" || v === "sharp";
}

export function isCoverShape(v: string | null | undefined): v is CoverShape {
  return v === "portrait" || v === "square";
}

export const STORE_LABELS: Record<Store, string> = {
  steam: "Steam",
  epic: "Epic",
  gog: "GOG",
  xbox: "Xbox",
  battlenet: "Battle.net",
  ubisoft: "Ubisoft",
  ea: "EA App",
  roblox: "Roblox",
  wargaming: "Wargaming",
  riot: "Riot",
  rockstar: "Rockstar",
  amazon: "Amazon",
  itch: "itch.io",
  humble: "Humble",
  manual: "Manual",
};

export const QUICK_FILTERS: { id: LibraryFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "favorites", label: "Favorites" },
  { id: "never", label: "Never" },
  { id: "missing", label: "Missing" },
];

export const STORE_FILTER_OPTIONS: { id: LibraryFilter; label: string }[] = [
  { id: "steam", label: "Steam" },
  { id: "epic", label: "Epic" },
  { id: "gog", label: "GOG" },
  { id: "xbox", label: "Xbox" },
  { id: "ea", label: "EA App" },
  { id: "battlenet", label: "Battle.net" },
  { id: "ubisoft", label: "Ubisoft" },
  { id: "wargaming", label: "Wargaming" },
  { id: "riot", label: "Riot" },
  { id: "rockstar", label: "Rockstar" },
  { id: "amazon", label: "Amazon" },
  { id: "itch", label: "itch.io" },
  { id: "humble", label: "Humble" },
  { id: "other", label: "Other" },
  { id: "hidden", label: "Hidden" },
];

export const SORT_OPTIONS: { id: SortMode; label: string }[] = [
  { id: "custom", label: "Custom" },
  { id: "name", label: "A–Z" },
  { id: "nameDesc", label: "Z–A" },
  { id: "recent", label: "Recently played" },
  { id: "added", label: "Recently added" },
  { id: "playtime", label: "Playtime" },
  { id: "favorites", label: "Favorites first" },
  { id: "missing", label: "Missing first" },
];

export const FILTER_OPTIONS = [...QUICK_FILTERS, ...STORE_FILTER_OPTIONS];

export function isSortMode(v: string | null | undefined): v is SortMode {
  return (
    v === "custom" ||
    v === "name" ||
    v === "nameDesc" ||
    v === "recent" ||
    v === "added" ||
    v === "playtime" ||
    v === "favorites" ||
    v === "missing"
  );
}

export function gameOrderKey(id: string) {
  return `game:${id}`;
}

export function groupOrderKey(id: string) {
  return `group:${id}`;
}

export function parseLibraryOrder(raw: string | null | undefined): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((x): x is string => typeof x === "string");
  } catch {
    return [];
  }
}

export function buildDefaultLibraryOrder(games: Game[], groups: GameGroup[]): string[] {
  const grouped = new Set(groups.flatMap((g) => g.gameIds));
  return [
    ...groups.map((g) => groupOrderKey(g.id)),
    ...games.filter((g) => !grouped.has(g.id)).map((g) => gameOrderKey(g.id)),
  ];
}

/** Merge saved order with current games/groups (append unknowns, drop missing). */
export function reconcileLibraryOrder(
  saved: string[],
  games: Game[],
  groups: GameGroup[],
): string[] {
  const grouped = new Set(groups.flatMap((g) => g.gameIds));
  const valid = new Set([
    ...groups.map((g) => groupOrderKey(g.id)),
    ...games.filter((g) => !grouped.has(g.id)).map((g) => gameOrderKey(g.id)),
  ]);
  const seen = new Set<string>();
  const next: string[] = [];
  for (const key of saved) {
    if (valid.has(key) && !seen.has(key)) {
      next.push(key);
      seen.add(key);
    }
  }
  for (const key of valid) {
    if (!seen.has(key)) next.push(key);
  }
  return next;
}

export function isThemeId(v: string | null | undefined): v is ThemeId {
  return (
    v === "emerald" ||
    v === "amber" ||
    v === "cyan" ||
    v === "slate" ||
    v === "crimson"
  );
}

/** Base card min width in px at scale 1. */
export const CARD_MIN_BASE = 150;

export function cardMinPx(scale: number): number {
  const s = Math.min(1.4, Math.max(0.7, scale || 1));
  return Math.round(CARD_MIN_BASE * s);
}

export interface AppearancePrefs {
  theme: ThemeId;
  cardScale: number;
  showTitles?: boolean;
  showStoreLabels?: boolean;
  gridDensity?: GridDensity;
  coverCorners?: CoverCorners;
  coverShape?: CoverShape;
  reduceMotion?: boolean;
}

function densityGap(d: GridDensity): string {
  if (d === "cozy") return "1.45rem";
  if (d === "compact") return "0.7rem";
  return "1.1rem";
}

function cornerRadius(c: CoverCorners): string {
  if (c === "round") return "22px";
  if (c === "sharp") return "4px";
  return "14px";
}

function shapeRatio(s: CoverShape): string {
  return s === "square" ? "1 / 1" : "2 / 3";
}

export function applyAppearance(prefs: AppearancePrefs | ThemeId, cardScaleArg?: number) {
  // Back-compat: older call sites used applyAppearance(theme, cardScale)
  const prefsObj: AppearancePrefs =
    typeof prefs === "string"
      ? { theme: prefs, cardScale: cardScaleArg ?? 1 }
      : prefs;

  const theme = prefsObj.theme;
  const cardScale = prefsObj.cardScale;
  const showTitles = prefsObj.showTitles !== false;
  const showStoreLabels = prefsObj.showStoreLabels !== false;
  const density = prefsObj.gridDensity ?? "normal";
  const corners = prefsObj.coverCorners ?? "soft";
  const shape = prefsObj.coverShape ?? "portrait";
  const reduceMotion = prefsObj.reduceMotion === true;

  document.documentElement.setAttribute("data-theme", theme);
  document.documentElement.style.setProperty("--card-min", `${cardMinPx(cardScale)}px`);
  document.documentElement.style.setProperty("--grid-gap", densityGap(density));
  document.documentElement.style.setProperty("--cover-radius", cornerRadius(corners));
  document.documentElement.style.setProperty("--cover-ratio", shapeRatio(shape));

  document.body.classList.toggle("hide-titles", !showTitles);
  document.body.classList.toggle("hide-store-labels", !showStoreLabels);
  document.body.classList.toggle("reduce-motion", reduceMotion);
}

export function appearanceFromSettings(s: AppSettings): AppearancePrefs {
  return {
    theme: isThemeId(s.theme) ? s.theme : "emerald",
    cardScale: typeof s.cardScale === "number" && s.cardScale > 0 ? s.cardScale : 1,
    showTitles: s.showTitles !== false,
    showStoreLabels: s.showStoreLabels !== false,
    gridDensity: isGridDensity(s.gridDensity) ? s.gridDensity : "normal",
    coverCorners: isCoverCorners(s.coverCorners) ? s.coverCorners : "soft",
    coverShape: isCoverShape(s.coverShape) ? s.coverShape : "portrait",
    reduceMotion: s.reduceMotion === true,
  };
}

export function formatPlaytime(minutes: number): string {
  if (!minutes || minutes <= 0) return "Never played";
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h < 100) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  return `${h}h`;
}

/** Where cover art came from (Steam, SteamGridDB, custom, …). */
export function coverSourceLabel(game: Game): string | null {
  const stored = game.coverSource?.trim();
  if (stored) return stored;
  const url = (game.coverUrl ?? "").toLowerCase();
  if (url.includes("steamstatic") || url.includes("steamcdn") || url.includes("steamcommunity")) {
    return "Steam";
  }
  if (url.includes("steamgriddb")) return "SteamGridDB";
  if (url.includes("wikipedia") || url.includes("wikimedia")) return "Wikipedia";
  if (url.includes("epicgames")) return "Epic";
  if (url.includes("xboxlive") || url.includes("microsoft.com")) return "Xbox";
  if (url.includes("roblox")) return "Roblox";
  if (!game.coverPath && !game.coverUrl) return null;
  if (game.steamAppId || game.store === "steam") return "Steam";
  if (game.store === "epic") return "Epic";
  if (game.store === "xbox") return "Xbox";
  if (game.store === "roblox") return "Roblox";
  return "Cached";
}

export function formatLastPlayed(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const now = new Date();
  const diff = now.getTime() - d.getTime();
  const days = Math.floor(diff / (1000 * 60 * 60 * 24));
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 14) return `${days} days ago`;
  return d.toLocaleDateString();
}

function steamIdOf(game: Game): string | null {
  if (game.steamAppId) return game.steamAppId;
  if (game.store === "steam" || game.id.startsWith("steam:")) {
    if (/^\d+$/.test(game.launchTarget)) return game.launchTarget;
    const m = /^steam:(\d+)/i.exec(game.id);
    if (m) return m[1];
  }
  return null;
}

function steamCoverUrls(appId: string): string[] {
  return [
    `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/${appId}/library_600x900.jpg`,
  ];
}

function fileBaseName(filePath: string): string | null {
  const name = filePath.split(/[/\\]/).pop()?.trim();
  return name ? name : null;
}

/** Local covers are served by the `cover` URI scheme (WebView2 often fails on asset://). */
export function localCoverUrl(filePath: string, bustCache = false): string | null {
  const name = fileBaseName(filePath);
  if (!name) return null;
  const q = bustCache ? `?t=${Date.now()}` : "";
  return `http://cover.localhost/${encodeURIComponent(name)}${q}`;
}

function localCoverUrls(filePath: string): string[] {
  const url = localCoverUrl(filePath);
  return url ? [url] : [];
}

/** Ordered cover URLs: local file via cover protocol, then Steam CDN, then stored URL. */
export function coverCandidates(
  game: Game,
  override?: string | null,
  opts?: { allowRemote?: boolean },
): string[] {
  const out: string[] = [];
  const add = (url?: string | null) => {
    if (!url || url.startsWith("data:")) return;
    if (!out.includes(url)) out.push(url);
  };
  add(override ?? null);
  if (game.coverPath) localCoverUrls(game.coverPath).forEach(add);
  if (opts?.allowRemote !== false) {
    const steamId = steamIdOf(game);
    if (steamId) steamCoverUrls(steamId).forEach(add);
    if (game.coverUrl && !isLikelyBrokenCoverUrl(game.coverUrl)) add(game.coverUrl);
  }
  return out;
}

/** Resolve a displayable cover URL. Local files use the asset protocol (not base64). */
export function coverSrc(game: Game, dataUrl?: string | null): string | null {
  return coverCandidates(game, dataUrl)[0] ?? null;
}

function isLikelyBrokenCoverUrl(url: string): boolean {
  const u = url.toLowerCase();
  return (
    u.includes("library_hero") ||
    u.includes("page_bg") ||
    u.includes("page_background") ||
    u.includes("header.jpg") ||
    u.includes("header_2x") ||
    u.includes("screenshot") ||
    u.includes("gameplay") ||
    u.includes("capsule_616") ||
    u.includes("capsule_467") ||
    u.includes("main_capsule") ||
    u.includes("banner") ||
    u.includes("1920x1080") ||
    u.includes("2560x1440")
  );
}

/** Load a local cover as a data URL (works even when asset protocol fails). */
export async function loadCoverDataUrl(gameId: string): Promise<string | null> {
  try {
    return await invoke<string | null>("get_cover_data_url", { id: gameId });
  } catch {
    return null;
  }
}
