import { useEffect, useState } from "react";
import { CoverChoiceGroup, localCoverUrl } from "../types";

interface Props {
  group: CoverChoiceGroup;
  index: number;
  total: number;
  onPick: (path: string) => void;
  onSkip: () => void;
  onBrowse?: () => void;
}

export function CoverPickModal({ group, index, total, onPick, onSkip, onBrowse }: Props) {
  const fallback = group.paths[0] ?? "";
  const initial =
    group.currentPath && group.paths.includes(group.currentPath) ? group.currentPath : fallback;
  const [selected, setSelected] = useState(initial);

  useEffect(() => {
    const next =
      group.currentPath && group.paths.includes(group.currentPath)
        ? group.currentPath
        : (group.paths[0] ?? "");
    setSelected(next);
  }, [group.gameId, group.currentPath, group.paths]);

  return (
    <div className="settings-backdrop cover-pick-backdrop" onClick={onSkip}>
      <div
        className="settings-panel settings-panel-wide cover-pick-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Choose cover"
      >
        <h2>Choose cover</h2>
        <p className="settings-lead">
          {group.name} has {group.paths.length} images
          {total > 1 ? ` · ${index + 1} of ${total}` : ""}. Pick the box art to use.
        </p>
        <div className="cover-pick-grid">
          {group.paths.map((path) => {
            const src = localCoverUrl(path);
            const active = selected === path;
            return (
              <button
                key={path}
                type="button"
                className={`cover-pick-card${active ? " active" : ""}`}
                onClick={() => setSelected(path)}
              >
                {src ? <img src={src} alt="" /> : <span>?</span>}
              </button>
            );
          })}
        </div>
        <div className="cover-pick-actions">
          {onBrowse ? (
            <button type="button" className="btn cover-pick-browse" onClick={onBrowse}>
              Browse…
            </button>
          ) : null}
          <button type="button" className="btn" onClick={onSkip}>
            Skip
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!selected}
            onClick={() => onPick(selected)}
          >
            Use this cover
          </button>
        </div>
      </div>
    </div>
  );
}
