import { useEffect, useMemo, useState } from "react";
import { Game, GameGroup, STORE_LABELS, Store, coverSrc } from "../types";

interface Props {
  open: boolean;
  group: GameGroup | null;
  members: Game[];
  allGames: Game[];
  coverMap: Record<string, string>;
  onClose: () => void;
  onAdd: (game: Game) => void;
}

export function GroupAddModal({
  open,
  group,
  members,
  allGames,
  coverMap,
  onClose,
  onAdd,
}: Props) {
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (open) setQuery("");
  }, [open, group?.id]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const memberIds = useMemo(() => new Set(members.map((m) => m.id)), [members]);
  const candidates = useMemo(() => {
    const q = query.trim().toLowerCase();
    return allGames
      .filter((g) => !g.hidden && !memberIds.has(g.id))
      .filter((g) => !q || g.name.toLowerCase().includes(q))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [allGames, memberIds, query]);

  if (!open || !group) return null;

  return (
    <div className="settings-backdrop" onClick={onClose} role="presentation">
      <div
        className="settings-panel group-picker-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Add games to group"
        onClick={(e) => e.stopPropagation()}
      >
        <h2>Add games</h2>
        <p className="settings-lead">Pick library games to add to “{group.name}”.</p>
        <div className="field">
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search games…"
            aria-label="Search games to add"
            autoFocus
          />
        </div>
        <div className="group-picker-list">
          {candidates.length === 0 ? (
            <p className="chart-empty">No matching games left to add.</p>
          ) : (
            candidates.map((game) => {
              const src = coverSrc(game, coverMap[game.id]);
              return (
                <button
                  key={game.id}
                  type="button"
                  className="group-picker-row"
                  onClick={() => onAdd(game)}
                >
                  <span className="group-picker-thumb">
                    {src ? (
                      <img src={src} alt="" draggable={false} />
                    ) : (
                      <span>{game.name.trim().charAt(0).toUpperCase() || "?"}</span>
                    )}
                  </span>
                  <span className="group-picker-text">
                    <span className="group-picker-name">{game.name}</span>
                    <span className="tile-store">
                      {STORE_LABELS[game.store as Store] ?? game.store}
                    </span>
                  </span>
                  <span className="group-picker-add">+</span>
                </button>
              );
            })
          )}
        </div>
        <div className="settings-actions">
          <button type="button" className="btn" onClick={onClose}>
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
