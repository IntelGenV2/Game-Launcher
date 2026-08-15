import { useEffect, useMemo, useState } from "react";
import { Game, GameGroup } from "../types";

interface Props {
  open: boolean;
  game: Game | null;
  groups: GameGroup[];
  onClose: () => void;
  onPick: (group: GameGroup) => void;
  onCreateGroup: () => void;
}

export function AddToGroupModal({
  open,
  game,
  groups,
  onClose,
  onPick,
  onCreateGroup,
}: Props) {
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (open) setQuery("");
  }, [open, game?.id]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const candidates = useMemo(() => {
    if (!game) return [];
    const q = query.trim().toLowerCase();
    return groups
      .filter((g) => !g.gameIds.includes(game.id))
      .filter((g) => !q || g.name.toLowerCase().includes(q))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [groups, game, query]);

  if (!open || !game) return null;

  return (
    <div className="settings-backdrop" onClick={onClose} role="presentation">
      <div
        className="settings-panel group-picker-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Add game to group"
        onClick={(e) => e.stopPropagation()}
      >
        <h2>Add to group</h2>
        <p className="settings-lead">Choose a group for “{game.name}”.</p>
        {groups.length === 0 ? (
          <p className="chart-empty">No groups yet.</p>
        ) : (
          <>
            <div className="field">
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search groups…"
                aria-label="Search groups"
                autoFocus
              />
            </div>
            <div className="group-picker-list">
              {candidates.length === 0 ? (
                <p className="chart-empty">No matching groups.</p>
              ) : (
                candidates.map((group) => (
                  <button
                    key={group.id}
                    type="button"
                    className="group-picker-row"
                    onClick={() => onPick(group)}
                  >
                    <span className="group-picker-thumb">
                      <span>{group.name.trim().charAt(0).toUpperCase() || "G"}</span>
                    </span>
                    <span className="group-picker-text">
                      <span className="group-picker-name">{group.name}</span>
                      <span className="tile-store">
                        {group.gameIds.length}{" "}
                        {group.gameIds.length === 1 ? "game" : "games"}
                      </span>
                    </span>
                    <span className="group-picker-add">+</span>
                  </button>
                ))
              )}
            </div>
          </>
        )}
        <div className="settings-actions">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              onClose();
              onCreateGroup();
            }}
          >
            Create group
          </button>
        </div>
      </div>
    </div>
  );
}
