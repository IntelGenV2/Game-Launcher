import { Game, STORE_LABELS, Store, coverSrc, formatLastPlayed, formatPlaytime } from "../types";
import { useEffect, useRef, useState } from "react";

interface Props {
  game: Game;
  coverDataUrl?: string | null;
  index: number;
  onOpen: (game: Game) => void;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onHide: (game: Game) => void;
  onOpenFolder: (game: Game) => void;
}

export function GameTile({
  game,
  coverDataUrl,
  index,
  onOpen,
  onLaunch,
  onToggleFavorite,
  onHide,
  onOpenFolder,
}: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [imgFailed, setImgFailed] = useState(false);
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

  const initial = game.name.trim().charAt(0).toUpperCase() || "?";

  return (
    <div
      className="tile"
      style={{ animationDelay: `${Math.min(index, 24) * 0.02}s` }}
      role="button"
      tabIndex={0}
      onClick={() => onOpen(game)}
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
            <button type="button" onClick={() => { onToggleFavorite(game); setMenuOpen(false); }}>
              {game.favorite ? "Remove favorite" : "Add favorite"}
            </button>
            <button
              type="button"
              onClick={() => { onOpenFolder(game); setMenuOpen(false); }}
              disabled={!game.installPath}
            >
              Open install folder
            </button>
            <button type="button" onClick={() => { onHide(game); setMenuOpen(false); }}>
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
