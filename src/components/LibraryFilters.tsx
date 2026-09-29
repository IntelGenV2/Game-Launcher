import type { CSSProperties } from "react";
import { QUICK_FILTERS, type LibraryFilter } from "../types";

export function LibraryFilters({ value, onChange }: { value: LibraryFilter; onChange: (value: LibraryFilter) => void }) {
  const index = Math.max(0, QUICK_FILTERS.findIndex(option => option.id === value));
  return <div className="library-filter-slider" role="radiogroup" aria-label="Quick filters"
    style={{ "--filter-index": index, "--filter-count": QUICK_FILTERS.length } as CSSProperties}>
    <span className="library-filter-indicator" aria-hidden="true" />
    {QUICK_FILTERS.map((option, i) => <button key={option.id} type="button" role="radio"
      aria-checked={i === index} tabIndex={i === index ? 0 : -1}
      onClick={() => onChange(option.id)}
      onKeyDown={event => {
        let next = i;
        if (event.key === "ArrowRight" || event.key === "ArrowDown") next = (i + 1) % QUICK_FILTERS.length;
        else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = (i + QUICK_FILTERS.length - 1) % QUICK_FILTERS.length;
        else if (event.key === "Home") next = 0;
        else if (event.key === "End") next = QUICK_FILTERS.length - 1;
        else return;
        event.preventDefault();
        event.stopPropagation();
        onChange(QUICK_FILTERS[next].id);
        event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>("button")[next]?.focus();
      }}>{option.label}</button>)}
  </div>;
}
