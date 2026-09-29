import type { Game, LibraryFilter, SortMode } from "./types";

export interface LibraryView {
  name: string;
  query: string;
  status: LibraryFilter;
  store: LibraryFilter;
  installedOnly: boolean;
  sort: SortMode;
  layout: "grid" | "list";
}

export function filterLibrary(games: Game[], query: string, status: LibraryFilter, store: LibraryFilter, installedOnly: boolean): Game[] {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return games.filter(game => {
    if (status === "hidden" ? !game.hidden : game.hidden) return false;
    if (status === "favorites" && !game.pinned) return false;
    if (status === "never" && (game.playtimeMinutes > 0 || game.lastPlayedAt)) return false;
    if (installedOnly && game.missing) return false;
    const stores = game.copies?.length ? game.copies.map(c => c.store) : [game.store];
    if (store === "other" ? !stores.some(s => s === "manual" || s === "roblox") : store !== "all" && !stores.some(s => s === store)) return false;
    const text = [game.name, ...(game.tags ?? []), ...(game.genres ?? [])].join(" ").toLocaleLowerCase();
    return words.every(word => text.includes(word));
  });
}

export function readSavedViews(raw?: string | null): LibraryView[] {
  try {
    const value: unknown = JSON.parse(raw ?? "[]");
    if (!Array.isArray(value)) return [];
    const statuses = ["all", "favorites", "never", "hidden"];
    const stores = ["all", "steam", "epic", "gog", "xbox", "ea", "battlenet", "ubisoft", "wargaming", "riot", "rockstar", "amazon", "itch", "humble", "other"];
    const sorts = ["custom", "name", "nameDesc", "recent", "added", "playtime", "favorites"];
    return value.filter((item): item is LibraryView => item && typeof item.name === "string" && item.name.length > 0 && item.name.length <= 60 && typeof item.query === "string" && statuses.includes(item.status) && stores.includes(item.store) && sorts.includes(item.sort) && typeof item.installedOnly === "boolean" && ["grid", "list"].includes(item.layout)).slice(0, 30);
  } catch { return []; }
}
