import { useEffect, useRef, useState } from "react";
import { Game, GameGroup } from "../types";
import { CoverImg } from "./CoverImg";
import {
  TileContextMenu,
  type MenuAnchor,
  anchorFromElement,
  anchorFromPoint,
} from "./TileContextMenu";

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
  const [menuAnchor, setMenuAnchor] = useState<MenuAnchor | null>(null);
  const suppressClick = useRef(false);
  const menuBtnRef = useRef<HTMLButtonElement>(null);
  const covers = members.slice(0, 3);

  function openMenu(anchor: MenuAnchor | null) {
    if (!anchor) return;
    setMenuAnchor(anchor);
    setMenuOpen(true);
  }

  function closeMenu() {
    setMenuOpen(false);
  }

  useEffect(() => {
    if (!menuOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
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
      onContextMenu={(e) => {
        e.preventDefault();
        openMenu(anchorFromPoint(e.clientX, e.clientY));
      }}
    >
      <div className="cover group-stack">
        <div className="badge-row">
          <span className="group-count">{members.length}</span>
          <button
            type="button"
            ref={menuBtnRef}
            className="menu-btn"
            title="More"
            onClick={(e) => {
              e.stopPropagation();
              if (menuOpen) closeMenu();
              else openMenu(anchorFromElement(menuBtnRef.current));
            }}
          >
            ⋯
          </button>
        </div>

        <div className="stack-layers" aria-hidden>
          {covers.length === 0 ? (
            <div className="cover-fallback stack-card">?</div>
          ) : (
            covers.map((g, i) => (
                <div
                  key={g.id}
                  className={`stack-card stack-card-${i}`}
                  style={{ zIndex: covers.length - i }}
                >
                  <CoverImg game={g} override={coverMap[g.id]} draggable={false} allowRemote={false} />
                </div>
            ))
          )}
        </div>
      </div>

      <TileContextMenu open={menuOpen} anchor={menuAnchor} onClose={closeMenu}>
        {onAddGames && (
          <button
            type="button"
            onClick={() => {
              onAddGames();
              closeMenu();
            }}
          >
            Add games…
          </button>
        )}
        <button
          type="button"
          onClick={() => {
            onRename(group);
            closeMenu();
          }}
        >
          Rename group
        </button>
        <button
          type="button"
          className="danger"
          onClick={() => {
            onDelete(group);
            closeMenu();
          }}
        >
          Delete group
        </button>
      </TileContextMenu>

      <div className="tile-title">{group.name}</div>
      <div className="tile-store">
        {members.length} {members.length === 1 ? "game" : "games"}
      </div>
    </div>
  );
}
