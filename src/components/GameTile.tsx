import { Game, GameGroup, STORE_LABELS, Store, formatLastPlayed, formatPlaytime } from "../types";
import { useEffect, useRef, useState } from "react";
import { CoverImg } from "./CoverImg";
import {
  TileContextMenu,
  type MenuAnchor,
  anchorFromElement,
  anchorFromPoint,
} from "./TileContextMenu";

interface Props {
  game: Game;
  coverDataUrl?: string | null;
  index: number;
  groups?: GameGroup[];
  inGroup?: GameGroup | null;
  groupAccent?: string | null;
  expandIndex?: number;
  dropActive?: boolean;
  dragActive?: boolean;
  onPointerDragStart?: (e: React.PointerEvent, game: Game) => void;
  onOpen: (game: Game) => void;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onHide: (game: Game) => void;
  onOpenFolder: (game: Game) => void;
  onOpenSaveFolder?: (game: Game) => void;
  onAddToGroup?: (game: Game) => void;
  onCreateGroup?: () => void;
  onRemoveFromGroup?: () => void;
  focusActive?: boolean;
  selectMode?: boolean;
  selected?: boolean;
  onToggleSelect?: (game: Game) => void;
}

export function GameTile({
  game,
  coverDataUrl,
  index,
  groups = [],
  inGroup = null,
  groupAccent = null,
  expandIndex,
  dropActive = false,
  dragActive = false,
  onPointerDragStart,
  onOpen,
  onLaunch,
  onToggleFavorite,
  onHide,
  onOpenFolder,
  onOpenSaveFolder,
  onAddToGroup,
  onCreateGroup,
  onRemoveFromGroup,
  focusActive = false,
  selectMode = false,
  selected = false,
  onToggleSelect,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<MenuAnchor | null>(null);
  const suppressClick = useRef(false);
  const menuBtnRef = useRef<HTMLButtonElement>(null);

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

  const style: React.CSSProperties = {
    animationDelay:
      expandIndex != null
        ? `${0.04 + expandIndex * 0.045}s`
        : `${Math.min(index, 24) * 0.02}s`,
  };
  if (groupAccent) {
    (style as Record<string, string>)["--group-accent"] = groupAccent;
  }

  return (
    <div
      className={`tile${inGroup ? " tile-in-group" : ""}${dropActive ? " drop-target" : ""}${
        dragActive ? " tile-dragging" : ""
      }${expandIndex != null ? " tile-expand-out" : ""}${focusActive ? " tile-focused" : ""}${
        selected ? " tile-selected" : ""
      }${selectMode ? " tile-select-mode" : ""}${menuOpen ? " menu-open" : ""}`}
      style={style}
      role="button"
      tabIndex={0}
      data-focus-key={`game:${game.id}`}
      data-drop-game={inGroup ? undefined : game.id}
      aria-selected={selectMode ? selected : undefined}
      onPointerDown={(e) => {
        if (selectMode) return;
        if (e.button !== 0) return;
        if ((e.target as HTMLElement).closest("button, .context-menu")) return;
        onPointerDragStart?.(e, game);
      }}
      onClick={() => {
        if (suppressClick.current) return;
        if (selectMode) {
          onToggleSelect?.(game);
          return;
        }
        onOpen(game);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          if (selectMode) onToggleSelect?.(game);
          else onOpen(game);
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        if (selectMode) return;
        openMenu(anchorFromPoint(e.clientX, e.clientY));
      }}
    >
      <div className="cover">
        {selectMode && (
          <span className={`select-check${selected ? " on" : ""}`} aria-hidden>
            {selected ? "✓" : ""}
          </span>
        )}
        <div className="badge-row">
          {!selectMode && (
            <>
              <button
                type="button"
                className={`star-btn${game.favorite ? " active" : ""}`}
                title={game.favorite ? "Unfavorite" : "Favorite"}
                onClick={(e) => {
                  e.stopPropagation();
                  onToggleFavorite(game);
                }}
              >
                ★
              </button>
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
            </>
          )}
        </div>

        <CoverImg game={game} override={coverDataUrl} draggable={false} allowRemote={false} />

        {game.missing && <span className="missing-badge">Missing</span>}
        {inGroup && <span className="group-badge">{inGroup.name}</span>}

        {!selectMode && (
          <div className="cover-overlay">
            <button
              type="button"
              className="play-btn"
              disabled={game.missing}
              aria-label={`Play ${game.name}`}
              onClick={(e) => {
                e.stopPropagation();
                onLaunch(game);
              }}
            >
              ▶
            </button>
            <span className="meta-line">{formatPlaytime(game.playtimeMinutes)}</span>
            <span className="meta-line">Last: {formatLastPlayed(game.lastPlayedAt)}</span>
          </div>
        )}
      </div>

      {!selectMode && (
        <TileContextMenu open={menuOpen} anchor={menuAnchor} onClose={closeMenu}>
          <button
            type="button"
            onClick={() => {
              onLaunch(game);
              closeMenu();
            }}
            disabled={game.missing}
          >
            Play
          </button>
          <button
            type="button"
            onClick={() => {
              onToggleFavorite(game);
              closeMenu();
            }}
          >
            {game.favorite ? "Remove favorite" : "Add favorite"}
          </button>
          {onOpenSaveFolder && game.saveFolder && (
            <button
              type="button"
              onClick={() => {
                onOpenSaveFolder(game);
                closeMenu();
              }}
            >
              Open save folder
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              onOpenFolder(game);
              closeMenu();
            }}
            disabled={!game.installPath}
          >
            Open install folder
          </button>
          {onRemoveFromGroup && (
            <button
              type="button"
              onClick={() => {
                onRemoveFromGroup();
                closeMenu();
              }}
            >
              Remove from group
            </button>
          )}
          {!inGroup && onAddToGroup && groups.length > 0 && (
            <button
              type="button"
              onClick={() => {
                onAddToGroup(game);
                closeMenu();
              }}
            >
              Add to group…
            </button>
          )}
          {!inGroup && groups.length === 0 && onCreateGroup && (
            <button
              type="button"
              onClick={() => {
                onCreateGroup();
                closeMenu();
              }}
            >
              Create group
            </button>
          )}
          <button
            type="button"
            onClick={() => {
              onHide(game);
              closeMenu();
            }}
          >
            {game.hidden ? "Unhide from library" : "Hide from library"}
          </button>
        </TileContextMenu>
      )}

      <div className="tile-title">{game.name}</div>
      <div className="tile-store">{STORE_LABELS[game.store as Store] ?? game.store}</div>
    </div>
  );
}
