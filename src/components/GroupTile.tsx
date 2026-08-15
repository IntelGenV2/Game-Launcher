import { useEffect, useRef, useState } from "react";
import { Game, GameGroup, coverSrc } from "../types";

interface Props {
  group: GameGroup;
  members: Game[];
  coverMap: Record<string, string>;
  index: number;
  dropActive?: boolean;
  dragActive?: boolean;
  onPointerDragStart?: (e: React.PointerEvent, group: GameGroup) => void;
  onOpen: () => void;
  onRename: (group: GameGroup) => void;
  onDelete: (group: GameGroup) => void;
  onAddGames?: () => void;
  focusActive?: boolean;
}

export function GroupTile({
  group,
  members,
  coverMap,
  index,
  dropActive = false,
  dragActive = false,
  onPointerDragStart,
  onOpen,
  onRename,
  onDelete,
  onAddGames,
  focusActive = false,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const suppressClick = useRef(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const covers = members.slice(0, 3);

  useEffect(() => {
    if (!menuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!menuRef.current?.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [menuOpen]);

  useEffect(() => {
    if (dragActive) suppressClick.current = true;
    else {
      const t = window.setTimeout(() => {
        suppressClick.current = false;
      }, 40);
      return () => window.clearTimeout(t);
    }
  }, [dragActive]);

  return (
    <div
      className={`tile group-tile${dropActive ? " drop-target" : ""}${
        dragActive ? " tile-dragging" : ""
      }${focusActive ? " tile-focused" : ""}${menuOpen ? " menu-open" : ""}`}
      style={{ animationDelay: `${Math.min(index, 24) * 0.02}s` }}
      role="button"
      tabIndex={0}
      data-focus-key={`group:${group.id}`}
      data-drop-group={group.id}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        if ((e.target as HTMLElement).closest("button, .context-menu")) return;
        onPointerDragStart?.(e, group);
      }}
      onClick={() => {
        if (suppressClick.current) return;
        onOpen();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onOpen();
        }
      }}
    >
      <div className="cover group-stack">
        <div className="badge-row">
          <span className="group-count">{members.length}</span>
          <button
            type="button"
            className="menu-btn"
            title="More"
            onClick={(e) => {
              e.stopPropagation();
              setMenuOpen((v) => !v);
            }}
          >
            ⋯
          </button>
        </div>

        <div className="stack-layers" aria-hidden>
          {covers.length === 0 ? (
            <div className="cover-fallback stack-card">?</div>
          ) : (
            covers.map((g, i) => {
              const src = coverSrc(g, coverMap[g.id]);
              return (
                <div
                  key={g.id}
                  className={`stack-card stack-card-${i}`}
                  style={{ zIndex: covers.length - i }}
                >
                  {src ? (
                    <img src={src} alt="" draggable={false} />
                  ) : (
                    <div className="cover-fallback">
                      {g.name.trim().charAt(0).toUpperCase() || "?"}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </div>
      </div>

      {menuOpen && (
        <div className="context-menu" ref={menuRef} onClick={(e) => e.stopPropagation()}>
          {onAddGames && (
            <button
              type="button"
              onClick={() => {
                onAddGames();
                setMenuOpen(false);
              }}
            >
              Add games…
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              onRename(group);
              setMenuOpen(false);
            }}
          >
            Rename group
          </button>
          <button
            type="button"
            className="danger"
            onClick={() => {
              onDelete(group);
              setMenuOpen(false);
            }}
          >
            Delete group
          </button>
        </div>
      )}

      <div className="tile-title">{group.name}</div>
      <div className="tile-store">
        {members.length} {members.length === 1 ? "game" : "games"}
      </div>
    </div>
  );
}
