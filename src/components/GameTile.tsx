import { Game, GameGroup, STORE_LABELS, Store, coverSrc, formatLastPlayed, formatPlaytime } from "../types";
import { useEffect, useRef, useState } from "react";

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
  onAddToGroup?: (game: Game, group: GameGroup) => void;
  onRemoveFromGroup?: () => void;
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
  onAddToGroup,
  onRemoveFromGroup,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [imgFailed, setImgFailed] = useState(false);
  const suppressClick = useRef(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const src = coverSrc(game, coverDataUrl);

  useEffect(() => {
    setImgFailed(false);
  }, [src, game.id, coverDataUrl]);

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

  const initial = game.name.trim().charAt(0).toUpperCase() || "?";
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
      }${expandIndex != null ? " tile-expand-out" : ""}`}
      style={style}
      role="button"
      tabIndex={0}
      data-drop-game={inGroup ? undefined : game.id}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        if ((e.target as HTMLElement).closest("button, .context-menu")) return;
        onPointerDragStart?.(e, game);
      }}
      onClick={() => {
        if (suppressClick.current) return;
        onOpen(game);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onOpen(game);
        }
      }}
    >
      <div className="cover">
        <div className="badge-row">
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

        {menuOpen && (
          <div className="context-menu" ref={menuRef} onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              onClick={() => {
                onToggleFavorite(game);
                setMenuOpen(false);
              }}
            >
              {game.favorite ? "Remove favorite" : "Add favorite"}
            </button>
            <button
              type="button"
              onClick={() => {
                onOpenFolder(game);
                setMenuOpen(false);
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
                  setMenuOpen(false);
                }}
              >
                Remove from group
              </button>
            )}
            {!inGroup && onAddToGroup && groups.length === 0 && (
              <button type="button" disabled>
                No groups yet — use Create group
              </button>
            )}
            {!inGroup &&
              onAddToGroup &&
              groups.map((g) => (
                <button
                  key={g.id}
                  type="button"
                  onClick={() => {
                    onAddToGroup(game, g);
                    setMenuOpen(false);
                  }}
                >
                  Add to “{g.name}”
                </button>
              ))}
            <button
              type="button"
              onClick={() => {
                onHide(game);
                setMenuOpen(false);
              }}
            >
              {game.hidden ? "Unhide from library" : "Hide from library"}
            </button>
          </div>
        )}

        {src && !imgFailed ? (
          <img src={src} alt="" loading="lazy" onError={() => setImgFailed(true)} draggable={false} />
        ) : (
          <div className="cover-fallback">{initial}</div>
        )}

        {game.missing && <span className="missing-badge">Missing</span>}
        {inGroup && <span className="group-badge">{inGroup.name}</span>}

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
      </div>
      <div className="tile-title">{game.name}</div>
      <div className="tile-store">{STORE_LABELS[game.store as Store] ?? game.store}</div>
    </div>
  );
}
