import { Game, GameGroup, STORE_LABELS, Store, formatLastPlayed, formatPlaytime } from "../types";
import { useEffect, useRef, useState } from "react";
import { CoverImg } from "./CoverImg";
import { gameBusy, launchLabel } from "../types";
import {
  TileContextMenu,
  type MenuAnchor,
  anchorFromElement,
  anchorFromPoint,
} from "./TileContextMenu";

interface Props {
  layout?: "grid" | "list";
  onTogglePin: (game: Game) => void;
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
  layout = "grid",
  onTogglePin,
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
      className={`tile${layout === "list" ? " tile-list" : ""}${inGroup ? " tile-in-group" : ""}${dropActive ? " drop-target" : ""}${
        dragActive ? " tile-dragging" : ""
      }${expandIndex != null ? " tile-expand-out" : ""}${focusActive ? " tile-focused" : ""}${
        selected ? " tile-selected" : ""
      }${selectMode ? " tile-select-mode" : ""}${menuOpen ? " menu-open" : ""}`}
      style={style}
      role="button"
      tabIndex={0}
      data-focus-key={`${inGroup ? "member" : "game"}:${game.id}`}
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
        if (e.target !== e.currentTarget) return;
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
                className={`star-btn${game.pinned ? " active" : ""}`}
                title={game.pinned ? "Unpin game" : "Pin game"}
                onClick={(e) => {
                  e.stopPropagation();
                  onTogglePin(game);
                }}
              >
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 2h8v2l-2 2v4l4 4v2h-5v5l-1 2-1-2v-5H6v-2l4-4V6L8 4z" /></svg>
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

        {(game.missing || gameBusy(game) || game.launchStatus?.state === "failed") && <span className="missing-badge" role="status">{gameBusy(game) ? launchLabel(game) : game.missing ? "Unavailable" : "Launch failed"}</span>}
        {game.pinned && <span className="pin-badge">Pinned</span>}
        {inGroup && <span className="group-badge">{inGroup.name}</span>}

        {!selectMode && (
          <div className="cover-overlay">
            <button
              type="button"
              className="play-btn"
              disabled={game.missing || gameBusy(game)}
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
            disabled={game.missing || gameBusy(game)}
          >
            {launchLabel(game)}
          </button>
          <button type="button" onClick={() => { onTogglePin(game); closeMenu(); }}>{game.pinned ? "Unpin game" : "Pin above games"}</button>
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
      {layout === "list" && <div className="tile-list-controls">
        <span className="list-time">{formatPlaytime(game.playtimeMinutes)}<small>{formatLastPlayed(game.lastPlayedAt)}</small></span>
        {!selectMode && <>
          <button type="button" className="btn" disabled={game.missing || gameBusy(game)} onClick={e => { e.stopPropagation(); onLaunch(game); }}>{launchLabel(game)}</button>
          <button type="button" className="btn" aria-label={`More actions for ${game.name}`} onClick={e => { e.stopPropagation(); openMenu(anchorFromElement(e.currentTarget)); }}>⋯</button>
        </>}
      </div>}
    </div>
  );
}
