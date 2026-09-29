import { useEffect, useRef, useState } from "react";
import { Game, STORE_LABELS } from "../types";
import { CoverImg } from "./CoverImg";

export function RandomPickModal({ games, coverMap, onPlay, onClose }: { games: Game[]; coverMap: Record<string,string>; onPlay: (game: Game) => void; onClose: () => void }) {
  const [id, setId] = useState(() => games[Math.floor(Math.random() * games.length)]?.id);
  const game = games.find(g => g.id === id) ?? games[0];
  const playRef = useRef<HTMLButtonElement>(null);
  const reroll = () => {
    const pool = games.filter(g => g.id !== game?.id);
    if (pool.length) setId(pool[Math.floor(Math.random() * pool.length)].id);
  };
  useEffect(() => { playRef.current?.focus(); }, []);
  if (!game) return null;
  return <div className="settings-backdrop" onClick={onClose}>
    <div className="settings-panel random-pick" role="dialog" aria-modal="true" aria-label="Pick something to play" onClick={e => e.stopPropagation()} onKeyDown={e => {
      if (e.key === "Escape") { e.stopPropagation(); onClose(); }
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") { e.preventDefault(); reroll(); }
      if (e.key === "Tab") { const buttons = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")); const first = buttons[0]; const last = buttons[buttons.length-1]; if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); } else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); } }
    }}>
      <h2>How about this?</h2>
      <p className="hint">Picked from {games.length} available games matching your current filters.</p>
      <div className="random-cover"><CoverImg key={game.id} game={game} override={coverMap[game.id]} loading="eager" /></div>
      <h3>{game.name}</h3><p className="hint">{STORE_LABELS[game.store]}</p>
      <div className="settings-actions"><button className="btn" onClick={onClose}>Cancel</button><button className="btn" disabled={games.length < 2} onClick={reroll}>Pick again</button><button ref={playRef} className="btn btn-primary" onClick={() => onPlay(game)}>Play</button></div>
    </div>
  </div>;
}
