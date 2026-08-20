import { useEffect, useMemo, useState } from "react";
import { Game, STORE_LABELS, Store, coverSrc } from "../types";

interface Props {
  open: boolean;
  games: Game[];
  groups: { key: string; games: Game[] }[];
  coverMap: Record<string, string>;
  onClose: () => void;
  onMerge: (keepId: string, sourceIds: string[]) => Promise<void>;
}

function preferredGame(list: Game[], store?: Store | null): Game {
  const byStore = store ? list.find((g) => g.store === store) : undefined;
  if (byStore) return byStore;
  return (
    list.find((g) => !g.missing && g.store === "steam") ??
    list.find((g) => !g.missing) ??
    list[0]
  );
}

export function MergeModal({ open, games, groups, coverMap, onClose, onMerge }: Props) {
  const manual = games.length >= 2;
  const [keepId, setKeepId] = useState<string | null>(null);
  const [picked, setPicked] = useState<Set<string>>(() => new Set());
  const [busy, setBusy] = useState(false);
  const [storeByKey, setStoreByKey] = useState<Record<string, Store>>({});

  const autoGroups = useMemo(() => (manual ? [] : groups.filter((g) => g.games.length >= 2)), [manual, groups]);

  useEffect(() => {
    if (!open) return;
    if (manual) {
      setPicked(new Set(games.map((g) => g.id)));
      setKeepId(preferredGame(games)?.id ?? null);
      return;
    }
    const next: Record<string, Store> = {};
    for (const g of groups) {
      next[g.key] = preferredGame(g.games).store;
    }
    setStoreByKey(next);
  }, [open, games, groups, manual]);

  if (!open) return null;

  function toggle(id: string) {
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const selected = games.filter((g) => picked.has(g.id));
  const keep = selected.find((g) => g.id === keepId) ?? selected[0];
  const sources = selected.filter((g) => g.id !== keep?.id);

  async function mergeAuto() {
    setBusy(true);
    try {
      for (const group of autoGroups) {
        const keepGame = preferredGame(group.games, storeByKey[group.key]);
        const sourceIds = group.games.filter((g) => g.id !== keepGame.id).map((g) => g.id);
        if (sourceIds.length === 0) continue;
        await onMerge(keepGame.id, sourceIds);
      }
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="settings-backdrop" onClick={() => !busy && onClose()}>
      <div
        className="settings-panel settings-panel-wide merge-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Merge duplicates"
      >
        {manual ? (
          <>
            <h2>Merge duplicates</h2>
            <p>Pick which copy to keep. Playtime, tags, notes, and art from the others fold into it.</p>
            <div className="merge-list">
              {games.map((g) => {
                const src = coverSrc(g, coverMap[g.id]);
                return (
                  <label key={g.id} className={`merge-row${picked.has(g.id) ? " on" : ""}`}>
                    <input type="checkbox" checked={picked.has(g.id)} onChange={() => toggle(g.id)} />
                    <span className="merge-thumb">
                      {src ? <img src={src} alt="" /> : <span>{g.name.charAt(0)}</span>}
                    </span>
                    <span className="merge-meta">
                      <strong>{g.name}</strong>
                      <span>
                        {STORE_LABELS[g.store as Store] ?? g.store}
                        {g.missing ? " · missing" : ""}
                      </span>
                    </span>
                    <label className="keep-radio">
                      <input
                        type="radio"
                        name="keep"
                        checked={keepId === g.id}
                        disabled={!picked.has(g.id)}
                        onChange={() => setKeepId(g.id)}
                      />
                      Keep
                    </label>
                  </label>
                );
              })}
            </div>
            <div className="settings-actions">
              <button type="button" className="btn" onClick={onClose} disabled={busy}>
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={busy || !keep || sources.length === 0}
                onClick={async () => {
                  if (!keep) return;
                  setBusy(true);
                  try {
                    await onMerge(
                      keep.id,
                      sources.map((s) => s.id),
                    );
                    onClose();
                  } finally {
                    setBusy(false);
                  }
                }}
              >
                {busy ? "Merging…" : `Merge ${sources.length + (keep ? 1 : 0)} games`}
              </button>
            </div>
          </>
        ) : (
          <>
            <h2>Same game, more than one store</h2>
            <p>Copies will merge automatically. Pick which store should launch each title.</p>
            <div className="auto-merge-list">
              {autoGroups.map((group) => {
                const stores = [...new Map(group.games.map((g) => [g.store, g])).values()];
                const sample = preferredGame(group.games, storeByKey[group.key]);
                const src = coverSrc(sample, coverMap[sample.id]);
                return (
                  <div key={group.key} className="auto-merge-row">
                    <span className="merge-thumb">
                      {src ? <img src={src} alt="" /> : <span>{sample.name.charAt(0)}</span>}
                    </span>
                    <div className="merge-meta">
                      <strong>{sample.name}</strong>
                      <div className="store-picks">
                        {stores.map((g) => (
                          <label key={g.id} className={storeByKey[group.key] === g.store ? "on" : ""}>
                            <input
                              type="radio"
                              name={`store-${group.key}`}
                              checked={storeByKey[group.key] === g.store}
                              onChange={() =>
                                setStoreByKey((prev) => ({ ...prev, [group.key]: g.store }))
                              }
                            />
                            {STORE_LABELS[g.store as Store] ?? g.store}
                            {g.missing ? " (missing)" : ""}
                          </label>
                        ))}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="settings-actions">
              <button type="button" className="btn" onClick={onClose} disabled={busy}>
                Not now
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={busy || autoGroups.length === 0}
                onClick={() => void mergeAuto()}
              >
                {busy ? "Merging…" : `Merge ${autoGroups.length} title${autoGroups.length === 1 ? "" : "s"}`}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
