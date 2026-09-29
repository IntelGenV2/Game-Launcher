import { useState } from "react";
import { LibraryView } from "../library";

export function SavedViewControls({ views, current, onApply, onSave }: { views: LibraryView[]; current: Omit<LibraryView, "name">; onApply: (view: LibraryView) => void; onSave: (views: LibraryView[]) => Promise<void> }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState("");
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function commit(next: LibraryView[]) {
    setBusy(true); setError(null);
    try { await onSave(next); setEditing(false); setName(""); setSelected(""); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  return <div className="saved-views">
    {views.length > 0 && <select aria-label="Saved views" value={selected} disabled={busy} onChange={e => { setSelected(e.target.value); const view = views.find(v => v.name === e.target.value); if (view) onApply(view); }}><option value="">Saved views</option>{views.map(view => <option key={view.name} value={view.name}>{view.name}</option>)}</select>}
    {selected && <button className="btn btn-subtle" disabled={busy} onClick={() => void commit(views.filter(v => v.name !== selected))}>Delete view</button>}
    {editing ? <form className="path-row" onSubmit={e => { e.preventDefault(); const trimmed = name.trim(); if (trimmed) void commit([...views.filter(v => v.name !== trimmed), { ...current, name: trimmed }]); }}>
      <input aria-label="View name" autoFocus maxLength={60} value={name} onChange={e => setName(e.target.value)} placeholder="Name this view" />
      <button className="btn" disabled={busy || !name.trim()} type="submit">Save</button><button className="btn" type="button" onClick={() => setEditing(false)}>Cancel</button>
    </form> : <button className="btn btn-subtle" disabled={views.length >= 30 || busy} onClick={() => setEditing(true)}>Save view</button>}
    {error && <span className="hint" role="alert">{error}</span>}
  </div>;
}
