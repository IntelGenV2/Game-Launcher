from pathlib import Path
p=Path('src/App.tsx')
s=p.read_text(encoding='utf-8')
s=s.replace('import { CustomSelect }', 'import { Titlebar } from "./components/Titlebar";\nimport { RandomPickModal } from "./components/RandomPickModal";\nimport { SavedViewControls } from "./components/SavedViewControls";\nimport { filterLibrary, readSavedViews, type LibraryView } from "./library";\nimport { gameBusy, type LaunchStatus } from "./types";\nimport { CustomSelect }',1)
s=s.replace('const [games, setGames] = useState<Game[]>([]);','''const [libraryGames, setGames] = useState<Game[]>([]);
  const [launchStates, setLaunchStates] = useState<Record<string, LaunchStatus>>({});
  const games = useMemo(() => libraryGames.map(game => ({ ...game, launchStatus: launchStates[game.id] })), [libraryGames, launchStates]);
  const [storeFilter, setStoreFilter] = useState<LibraryFilter>("all");
  const [installedOnly, setInstalledOnly] = useState(false);
  const [randomOpen, setRandomOpen] = useState(false);''')
s=s.replace('  const sessionStarts = useRef<Record<string, number>>({});\n','')
start=s.index('    const q = query.trim().toLowerCase();',s.index('  const filtered = useMemo'))
end=s.index('    list = [...list].sort',start)
s=s[:start]+'''    let list = filterLibrary(games, query, libraryFilter, storeFilter, installedOnly);

'''+s[end:]
s=s.replace('}, [games, query, libraryFilter, sortBy, libraryOrder]);','}, [games, query, libraryFilter, storeFilter, installedOnly, sortBy, libraryOrder]);',1)
start=s.index('  async function handleLaunch(game: Game)')
end=s.index('  async function handleToggleFavorite(',start)
s=s[:start]+'''  async function handleLaunch(game: Game, sourceId?: string) {
    if (gameBusy(game)) return;
    if (game.missing) { openGame(game); return; }
    setLaunchStates(prev => ({ ...prev, [game.id]: { gameId: game.id, sourceId: sourceId ?? game.id, state: "launching" } }));
    try {
      await invoke<Game>("launch_game", { id: game.id, sourceId: sourceId ?? null });
    } catch (e) {
      setLaunchStates(prev => ({ ...prev, [game.id]: { gameId: game.id, sourceId: sourceId ?? game.id, state: "failed", message: String(e) } }));
      showToast(String(e), true);
    }
  }

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    const received = new Set<string>();
    void (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const stop = await listen<LaunchStatus>("launch-status", event => {
          if (cancelled) return;
          const status = event.payload;
          received.add(status.gameId);
          setLaunchStates(prev => ({ ...prev, [status.gameId]: status }));
          if (status.state === "failed") showToast(status.message ?? "Could not launch the game", true);
          if (status.state === "running" || status.state === "finished") {
            void invoke<Game>("get_game", { id: status.gameId }).then(updated => {
              if (!cancelled) setGames(prev => prev.map(game => game.id === updated.id ? asLibraryGame(updated) : game));
            }).catch(() => undefined);
            void refreshStats();
            setOverview(null);
          }
        });
        if (cancelled) { stop(); return; }
        unlisten = stop;
        const snapshot = await invoke<LaunchStatus[]>("launch_statuses");
        if (!cancelled) setLaunchStates(prev => ({ ...Object.fromEntries(snapshot.filter(status => !received.has(status.gameId)).map(status => [status.gameId, status])), ...prev }));
      } catch { /* Browser preview has no native session service. */ }
    })();
    return () => { cancelled = true; unlisten?.(); };
  }, [showToast, refreshStats]);

  async function handleTogglePin(game: Game) {
    try {
      const updated = await invoke<Game>("set_pinned", { id: game.id, pinned: !game.pinned });
      setGames(prev => prev.map(item => item.id === updated.id ? asLibraryGame(updated) : item));
      showToast(updated.pinned ? "Pinned above your games" : "Game unpinned");
    } catch (error) { showToast(String(error), true); }
  }

'''+s[end:]
start=s.index('  function handleRandomLaunch()')
end=s.index('  async function toggleBigPicture()',start)
s=s[:start]+'''  const randomPool = filtered.filter(game => !game.missing && !gameBusy(game));
  function handleRandomLaunch() {
    if (!randomPool.length) { showToast("No available games in this view", true); return; }
    setRandomOpen(true);
  }

  async function saveViewSettings(changes: Partial<AppSettings>) {
    const next = { ...settings, ...changes };
    await invoke("save_settings", { settings: next });
    setSettings(next);
  }

  const layout = settings.libraryLayout === "list" ? "list" : "grid";
  function applySavedView(view: LibraryView) {
    setQuery(view.query); setLibraryFilter(view.status); setStoreFilter(view.store);
    setInstalledOnly(view.installedOnly); setSortBy(view.sort);
    void saveViewSettings({ libraryLayout: view.layout, sortBy: view.sort }).catch(error => showToast(String(error), true));
  }

'''+s[end:]
s=s.replace('  const modalOpen =\n    settingsOpen ||','  const modalOpen =\n    randomOpen || systemOpen || settingsOpen ||')
s=s.replace('key.startsWith("game:")', '(key.startsWith("game:") || key.startsWith("member:"))')
s=s.replace('key?.startsWith("game:")','(key?.startsWith("game:") || key?.startsWith("member:"))')
s=s.replace('const id = key.slice(5);','const id = key.slice(key.indexOf(":") + 1);')
s=s.replace('x.id === key.slice(5)', 'x.id === key.slice(key.indexOf(":") + 1)')
s=s.replace('          setSettingsOpen(false);','          setRandomOpen(false);\n          setSystemOpen(false);\n          setSettingsOpen(false);')
s=s.replace('      if (selectedGame) {\n        if (e.key === "Enter")', '      if ((e.target as HTMLElement).closest("button, select, summary, [role=dialog]")) return;\n\n      if (selectedGame) {\n        if (e.key === "Enter")')
s=s.replace('  const storeFilterActive = STORE_FILTER_OPTIONS.some((o) => o.id === libraryFilter);','  const storeFilterActive = storeFilter !== "all";')
s=s.replace('    <div\n      className={`app-shell', '    <div className={`app-window${bigPicture ? " bp-open" : ""}`}>\n      <Titlebar onError={showToast} />\n    <div\n      className={`app-shell',1)
s=s.replace('        <div className="sidebar-foot">\n          <button','        <div className="sidebar-foot">\n          {settings.showTools && <button',1)
s=s.replace('            System\n          </button>', '            Tools\n          </button>}',1)
s=s.replace('s.id === libraryFilter)?.label ?? "Stores"','s.id === storeFilter)?.label ?? "Stores"')
s=s.replace('{STORE_FILTER_OPTIONS.map((s) => (','{[{ id: "all" as LibraryFilter, label: "All stores" }, ...STORE_FILTER_OPTIONS].map((s) => (',1)
s=s.replace('className={libraryFilter === s.id ? "active" : ""}','className={storeFilter === s.id ? "active" : ""}',1)
s=s.replace('                            setLibraryFilter(s.id);','                            setStoreFilter(s.id);',1)
s=s.replace('          <div className="top-actions">','          <div className="top-actions">',1)
s=s.replace('        <main className="main">','''        {mainView === "library" && !selectedGame && <div className="library-viewbar">
          <span className="library-count">{filtered.length} games</span>
          <label className="inline-check"><input type="checkbox" checked={installedOnly} onChange={e => setInstalledOnly(e.target.checked)} />Installed only</label>
          {(query || libraryFilter !== "all" || storeFilter !== "all" || installedOnly) && <button className="btn btn-subtle" onClick={() => { setQuery(""); setLibraryFilter("all"); setStoreFilter("all"); setInstalledOnly(false); }}>Clear filters</button>}
          <SavedViewControls views={readSavedViews(settings.savedViews)} current={{ query, status: libraryFilter, store: storeFilter, installedOnly, sort: sortBy, layout }} onApply={applySavedView} onSave={views => saveViewSettings({ savedViews: JSON.stringify(views) })} />
          <div className="view-toggle" aria-label="Library layout">{(["grid", "list"] as const).map(mode => <button key={mode} className={`chip-btn${layout === mode ? " active" : ""}`} aria-pressed={layout === mode} onClick={() => void saveViewSettings({ libraryLayout: mode }).catch(error => showToast(String(error), true))}>{mode === "grid" ? "Grid" : "List"}</button>)}</div>
        </div>}
        <main className="main">''',1)
s=s.replace('                <GameGrid\n','                <GameGrid\n                  layout={layout}\n                  onTogglePin={handleTogglePin}\n',1)
s=s.replace('            <GameDetail\n','            <GameDetail\n              onTogglePin={handleTogglePin}\n              onRescan={() => void handleRescan()}\n',1)
s=s.replace('          showToast(`Merged into ${kept.name}`);','          await refreshGroups();\n          showToast(`Linked copies of ${kept.name}`);')
s=s.replace('            reduceMotion={settings.reduceMotion === true}','            showTools={settings.showTools === true}\n            reduceMotion={settings.reduceMotion === true}')
s=s.replace('        onResetArt={async () => {\n          await invoke("reset_all_art");','        onResetArt={async (custom: boolean) => {\n          await invoke("reset_art", { custom });')
s=s.replace('          showToast("Cover art reset — fetching new art");','          showToast(custom ? "Custom artwork removed" : "Downloaded artwork refreshed");')
s=s.replace('          sessionStarts.current = {};\n','')
s=s.replace('      <UpdateChecker />','      {randomOpen && randomPool.length > 0 && <RandomPickModal games={randomPool} onClose={() => setRandomOpen(false)} onPlay={game => { setRandomOpen(false); void handleLaunch(game); }} />}\n\n      <UpdateChecker />')
s=s.replace('    </div>\n  );\n}\n\nexport default App;', '    </div>\n    </div>\n  );\n}\n\nexport default App;')
p.write_text(s,encoding='utf-8')
