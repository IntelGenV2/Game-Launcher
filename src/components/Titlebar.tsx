import { useEffect, useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function Titlebar({ children, onError }: { children?: ReactNode; onError: (message: string, error?: boolean) => void }) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const sync = () => void win.isMaximized().then(value => { if (!cancelled) setMaximized(value); }).catch(() => undefined);
    sync();
    void win.onResized(sync).then(fn => { if (cancelled) fn(); else unlisten = fn; }).catch(() => undefined);
    return () => { cancelled = true; unlisten?.(); };
  }, []);
  const run = (action: () => Promise<unknown>) => { if (isTauri()) void action().catch(error => onError(String(error), true)); };
  return <header className="titlebar" onMouseDown={event => {
    if (event.button !== 0 || !(event.target instanceof Element)) return;
    // Descendant layout containers are drag surfaces too; actual controls keep their input.
    if (event.target.closest("button, input, select, textarea, a, [role=listbox], [role=menu], [data-no-drag]")) return;
    event.preventDefault();
    run(() => event.detail === 2 ? getCurrentWindow().toggleMaximize() : getCurrentWindow().startDragging());
  }}>
    <div className="brand titlebar-brand">
      <img src="/intelgen-icon.png" className="brand-mark" alt="" draggable={false} />
      <div className="titlebar-wordmark"><h1 className="brand-name">IntelGen</h1><span className="brand-subtitle">Game Launcher</span></div>
    </div>
    <div className="titlebar-content">{children}</div>
    <div className="titlebar-controls">
      <button type="button" aria-label="Minimize" title="Minimize" onClick={() => run(() => getCurrentWindow().minimize())}><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8" /></svg></button>
      <button type="button" aria-label={maximized ? "Restore window" : "Maximize"} title={maximized ? "Restore" : "Maximize"} onClick={() => run(() => getCurrentWindow().toggleMaximize())}><svg viewBox="0 0 12 12" aria-hidden="true">{maximized ? <path d="M3 4h5v5H3zM5 2h5v5" /> : <path d="M2.5 2.5h7v7h-7z" />}</svg></button>
      <button type="button" className="titlebar-close" aria-label="Close" title="Close" onClick={() => run(() => getCurrentWindow().close())}><svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" /></svg></button>
    </div>
  </header>;
}
