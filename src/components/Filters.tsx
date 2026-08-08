import { Store, STORE_LABELS } from "../types";

const STORES: Store[] = [
  "steam",
  "epic",
  "gog",
  "xbox",
  "ea",
  "battlenet",
  "ubisoft",
  "roblox",
  "manual",
];

interface Props {
  activeStores: Set<Store>;
  favoritesOnly: boolean;
  showHidden: boolean;
  onToggleStore: (store: Store) => void;
  onToggleFavorites: () => void;
  onToggleHidden: () => void;
  onClearStores: () => void;
}

export function Filters({
  activeStores,
  favoritesOnly,
  showHidden,
  onToggleStore,
  onToggleFavorites,
  onToggleHidden,
  onClearStores,
}: Props) {
  return (
    <div className="filters">
      <button
        type="button"
        className={`chip favor${favoritesOnly ? " active" : ""}`}
        onClick={onToggleFavorites}
      >
        ★ Favorites
      </button>
      <button
        type="button"
        className={`chip${showHidden ? " active" : ""}`}
        onClick={onToggleHidden}
      >
        Hidden
      </button>
      <button
        type="button"
        className={`chip${activeStores.size === 0 && !showHidden ? " active" : ""}`}
        onClick={onClearStores}
      >
        All stores
      </button>
      {STORES.map((store) => (
        <button
          key={store}
          type="button"
          className={`chip${activeStores.has(store) ? " active" : ""}`}
          onClick={() => onToggleStore(store)}
        >
          {STORE_LABELS[store]}
        </button>
      ))}
    </div>
  );
}
