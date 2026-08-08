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
  | "manual";

export interface Game {
  id: string;
  name: string;
  store: Store;
  launchTarget: string;
  installPath: string | null;
  coverUrl: string | null;
  coverPath: string | null;
  favorite: boolean;
  hidden: boolean;
  missing: boolean;
  playtimeMinutes: number;
  lastPlayedAt: string | null;
  dateAdded: string;
  steamAppId: string | null;
}

export interface AppSettings {
  steamGridDbApiKey: string | null;
  sortBy: string | null;
}

export interface LibraryStats {
  total: number;
  favorites: number;
  missing: number;
}

export interface PlaySession {
  id: number;
  gameId: string;
  startedAt: string;
  endedAt: string | null;
  durationMinutes: number;
  avgFps: number | null;
}

export interface FpsSample {
  id: number;
  gameId: string;
  recordedAt: string;
  fps: number;
  note: string | null;
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
  avgFps: number | null;
  latestFps: number | null;
  dailyPlaytime: DailyPlaytime[];
  sessions: PlaySession[];
  fpsSamples: FpsSample[];
}

export type SortMode = "name" | "recent" | "playtime" | "favorites";

export const STORE_LABELS: Record<Store, string> = {
  steam: "Steam",
  epic: "Epic",
  gog: "GOG",
  xbox: "Xbox",
  battlenet: "Battle.net",
  ubisoft: "Ubisoft",
  ea: "EA App",
  roblox: "Roblox",
  manual: "Manual",
};

export function formatPlaytime(minutes: number): string {
  if (!minutes || minutes <= 0) return "Never played";
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h < 100) return m > 0 ? `${h}h ${m}m` : `${h}h`;
  return `${h}h`;
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

/** Resolve a displayable cover URL. Local cached art (data URL) wins. */
export function coverSrc(game: Game, dataUrl?: string | null): string | null {
  if (dataUrl) return dataUrl;
  // Wait for local data-URL load when we have a cached file — asset protocol is unreliable,
  // and Steam CDN library_600x900 404s for many modern titles (PEAK, RV There Yet?, …).
  if (game.coverPath) {
    return null;
  }
  if (game.steamAppId) {
    return `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/${game.steamAppId}/library_600x900.jpg`;
  }
  if (game.coverUrl && !isLikelyBrokenCoverUrl(game.coverUrl)) {
    return game.coverUrl;
  }
  return null;
}

function isLikelyBrokenCoverUrl(url: string): boolean {
  const u = url.toLowerCase();
  return (
    u.includes("header.jpg") ||
    u.includes("library_hero") ||
    u.includes("page_bg") ||
    u.includes("capsule_616")
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
