import { Game } from "../types";
import { GameTile } from "./GameTile";

interface Props {
  games: Game[];
  coverMap: Record<string, string>;
  onOpen: (game: Game) => void;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onHide: (game: Game) => void;
  onOpenFolder: (game: Game) => void;
}

export function GameGrid({
  games,
  coverMap,
  onOpen,
  onLaunch,
  onToggleFavorite,
  onHide,
  onOpenFolder,
}: Props) {
  if (games.length === 0) {
    return (
      <div className="empty">
        <h2>No games match</h2>
        <p>Try clearing filters or running a rescan.</p>
      </div>
    );
  }

  return (
    <div className="game-grid">
      {games.map((game, index) => (
        <GameTile
          key={game.id}
          game={game}
          index={index}
          coverDataUrl={coverMap[game.id]}
          onOpen={onOpen}
          onLaunch={onLaunch}
          onToggleFavorite={onToggleFavorite}
          onHide={onHide}
          onOpenFolder={onOpenFolder}
        />
      ))}
    </div>
  );
}
