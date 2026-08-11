import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  LibraryDrag,
  LibraryDropTarget,
  PointerDragSession,
  dropTargetKey,
  libraryDragKey,
  resolveDropTarget,
  shouldActivateDrag,
} from "../dnd";
import {
  Game,
  GameGroup,
  coverSrc,
  gameOrderKey,
  groupOrderKey,
  reconcileLibraryOrder,
} from "../types";
import { GameTile } from "./GameTile";
import { GroupTile } from "./GroupTile";

interface Props {
  games: Game[];
  groups: GameGroup[];
  coverMap: Record<string, string>;
  libraryOrder: string[];
  expandedGroupId: string | null;
  onExpandedGroupChange: (id: string | null) => void;
  onOpen: (game: Game) => void;
  onLaunch: (game: Game) => void;
  onToggleFavorite: (game: Game) => void;
  onHide: (game: Game) => void;
  onOpenFolder: (game: Game) => void;
  onAddGame: () => void;
  onRenameGroup: (group: GameGroup) => void;
  onDeleteGroup: (group: GameGroup) => void;
  onAddToGroup: (game: Game, group: GameGroup) => void;
  onRemoveFromGroup: (group: GameGroup, game: Game) => void;
  onReorder: (nextOrder: string[]) => void;
  onAddGamesToGroup: (group: GameGroup) => void;
}

function groupAccent(id: string): string {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360} 62% 52%)`;
}

type DragPreview = {
  x: number;
  y: number;
  offsetX: number;
  offsetY: number;
  width: number;
  title: string;
  cover: string | null;
  initial: string;
  stacked?: { cover: string | null; initial: string }[];
};

export function GameGrid({
  games,
  groups,
  coverMap,
  libraryOrder,
  expandedGroupId,
  onExpandedGroupChange,
  onOpen,
  onLaunch,
  onToggleFavorite,
  onHide,
  onOpenFolder,
  onAddGame,
  onRenameGroup,
  onDeleteGroup,
  onAddToGroup,
  onRemoveFromGroup,
  onReorder,
  onAddGamesToGroup,
}: Props) {
  const gamesGridRef = useRef<HTMLDivElement>(null);
  const [cols, setCols] = useState(1);
  const [drag, setDrag] = useState<LibraryDrag | null>(null);
  const [dropTarget, setDropTarget] = useState<LibraryDropTarget>(null);
  const [preview, setPreview] = useState<DragPreview | null>(null);
  const sessionRef = useRef<PointerDragSession | null>(null);
  const dragRef = useRef<LibraryDrag | null>(null);
  const dropRef = useRef<LibraryDropTarget>(null);
  const orderRef = useRef<string[]>([]);
  const gameByIdRef = useRef<Map<string, Game>>(new Map());
  const groupByIdRef = useRef<Map<string, GameGroup>>(new Map());
  const coverMapRef = useRef(coverMap);
  const callbacksRef = useRef({ onAddToGroup, onReorder });
  coverMapRef.current = coverMap;

  const gameById = useMemo(() => new Map(games.map((g) => [g.id, g])), [games]);
  const groupById = useMemo(() => new Map(groups.map((g) => [g.id, g])), [groups]);
  const order = useMemo(
    () => reconcileLibraryOrder(libraryOrder, games, groups),
    [libraryOrder, games, groups],
  );

  orderRef.current = order;
  gameByIdRef.current = gameById;
  groupByIdRef.current = groupById;
  callbacksRef.current = { onAddToGroup, onReorder };
  dragRef.current = drag;
  dropRef.current = dropTarget;

  const groupedIds = useMemo(() => new Set(groups.flatMap((g) => g.gameIds)), [groups]);

  const groupItems = useMemo(() => {
    const keys = order.filter((k) => k.startsWith("group:"));
    const items: {
      group: GameGroup;
      members: Game[];
      expanded: boolean;
      accent: string;
    }[] = [];
    for (const key of keys) {
      const group = groupById.get(key.slice(6));
      if (!group) continue;
      const members = group.gameIds
        .map((id) => gameById.get(id))
        .filter((g): g is Game => Boolean(g));
      const visible = members.filter((m) => games.some((g) => g.id === m.id));
      if (members.length > 0 && visible.length === 0) continue;
      items.push({
        group,
        members: visible,
        expanded: expandedGroupId === group.id,
        accent: groupAccent(group.id),
      });
    }
    return items;
  }, [order, groupById, gameById, games, expandedGroupId]);

  const ungroupedGames = useMemo(() => {
    const keys = order.filter((k) => k.startsWith("game:"));
    const list: Game[] = [];
    for (const key of keys) {
      const game = gameById.get(key.slice(5));
      if (!game || groupedIds.has(game.id)) continue;
      if (!games.some((g) => g.id === game.id)) continue;
      list.push(game);
    }
    return list;
  }, [order, gameById, groupedIds, games]);

  useLayoutEffect(() => {
    const el = gamesGridRef.current;
    if (!el) return;
    const measure = () => {
      const style = getComputedStyle(el);
      const gap = parseFloat(style.columnGap || style.gap || "0") || 0;
      const minRaw = getComputedStyle(document.documentElement)
        .getPropertyValue("--card-min")
        .trim();
      const min = parseFloat(minRaw) || 150;
      setCols(Math.max(1, Math.floor((el.clientWidth + gap) / (min + gap))));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    const mo = new MutationObserver(measure);
    mo.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["style", "data-theme"],
    });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, [ungroupedGames.length, groupItems.length]);

  useEffect(() => {
    document.body.classList.toggle("library-dragging", drag != null);
    return () => document.body.classList.remove("library-dragging");
  }, [drag]);

  function moveKey(fromKey: string, beforeKey: string | null) {
    const current = orderRef.current;
    if (!fromKey.startsWith("game:") && !fromKey.startsWith("group:")) return;
    const next = [...current];
    const fromIdx = next.indexOf(fromKey);
    if (fromIdx < 0) return;
    next.splice(fromIdx, 1);
    if (!beforeKey) next.push(fromKey);
    else {
      let toIdx = next.indexOf(beforeKey);
      if (toIdx < 0) toIdx = next.length;
      next.splice(toIdx, 0, fromKey);
    }
    callbacksRef.current.onReorder(next);
  }

  function commitDrop(payload: LibraryDrag, target: LibraryDropTarget) {
    if (!target) return;
    if (target.kind === "group") {
      const group = groupByIdRef.current.get(target.groupId);
      if (!group) return;
      if (payload.type === "game") {
        const game = gameByIdRef.current.get(payload.gameId);
        if (game && !group.gameIds.includes(game.id)) {
          callbacksRef.current.onAddToGroup(game, group);
        }
        return;
      }
      if (payload.type === "group" && payload.groupId !== group.id) {
        moveKey(groupOrderKey(payload.groupId), groupOrderKey(group.id));
      }
      return;
    }
    if (target.kind === "game") {
      const before = gameOrderKey(target.gameId);
      if (payload.type === "game" && payload.gameId !== target.gameId) {
        moveKey(gameOrderKey(payload.gameId), before);
      } else if (payload.type === "group") {
        moveKey(groupOrderKey(payload.groupId), before);
      }
      return;
    }
    if (target.kind === "end") {
      if (payload.type === "game") moveKey(gameOrderKey(payload.gameId), null);
      if (payload.type === "group") moveKey(groupOrderKey(payload.groupId), null);
    }
  }

  function beginPointerSession(e: React.PointerEvent, payload: LibraryDrag) {
    if (sessionRef.current) return;
    const tile = (e.currentTarget as HTMLElement).getBoundingClientRect();
    sessionRef.current = {
      drag: payload,
      active: false,
      startX: e.clientX,
      startY: e.clientY,
      pointerId: e.pointerId,
      offsetX: e.clientX - tile.left,
      offsetY: e.clientY - tile.top,
      width: tile.width,
    };

    const buildPreview = (x: number, y: number, session: PointerDragSession): DragPreview => {
      if (session.drag.type === "game") {
        const game = gameByIdRef.current.get(session.drag.gameId);
        const title = game?.name ?? "Game";
        const cover = game ? coverSrc(game, coverMapRef.current[game.id]) : null;
        const initial = title.trim().charAt(0).toUpperCase() || "?";
        return {
          x,
          y,
          offsetX: session.offsetX,
          offsetY: session.offsetY,
          width: session.width,
          title,
          cover,
          initial,
        };
      }
      const group = groupByIdRef.current.get(session.drag.groupId);
      const members = (group?.gameIds ?? [])
        .map((id) => gameByIdRef.current.get(id))
        .filter((g): g is Game => Boolean(g))
        .slice(0, 3);
      return {
        x,
        y,
        offsetX: session.offsetX,
        offsetY: session.offsetY,
        width: session.width,
        title: group?.name ?? "Group",
        cover: members[0] ? coverSrc(members[0], coverMapRef.current[members[0].id]) : null,
        initial: (group?.name ?? "?").trim().charAt(0).toUpperCase() || "?",
        stacked: members.map((g) => ({
          cover: coverSrc(g, coverMapRef.current[g.id]),
          initial: g.name.trim().charAt(0).toUpperCase() || "?",
        })),
      };
    };

    const onMove = (ev: PointerEvent) => {
      const session = sessionRef.current;
      if (!session || ev.pointerId !== session.pointerId) return;
      if (!session.active) {
        if (!shouldActivateDrag(session, ev.clientX, ev.clientY)) return;
        session.active = true;
        setDrag(session.drag);
        setPreview(buildPreview(ev.clientX, ev.clientY, session));
        document.body.style.userSelect = "none";
      } else {
        setPreview((prev) =>
          prev
            ? { ...prev, x: ev.clientX, y: ev.clientY }
            : buildPreview(ev.clientX, ev.clientY, session),
        );
      }
      const next = resolveDropTarget(ev.clientX, ev.clientY, session.drag);
      setDropTarget(next);
    };

    const finish = (ev: PointerEvent) => {
      const session = sessionRef.current;
      if (!session || ev.pointerId !== session.pointerId) return;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      document.body.style.userSelect = "";
      const wasActive = session.active;
      const payload = session.drag;
      const target = dropRef.current;
      sessionRef.current = null;
      setDrag(null);
      setDropTarget(null);
      setPreview(null);
      if (wasActive) commitDrop(payload, target);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  const remainder = ungroupedGames.length % cols;
  const ghosts =
    ungroupedGames.length === 0 && groupItems.length === 0
      ? 0
      : remainder === 0
        ? 1
        : cols - remainder;

  if (groupItems.length === 0 && ungroupedGames.length === 0) {
    return (
      <div className="empty">
        <h2>No games match</h2>
        <p>Try clearing filters or running a rescan.</p>
      </div>
    );
  }

  const activeKey = drag ? libraryDragKey(drag) : null;
  const dropKey = dropTargetKey(dropTarget);

  return (
    <div className="library-sections">
      {groupItems.length > 0 && (
        <section className="library-section groups-section">
          <h3 className="section-label">Groups</h3>
          <div className="game-grid groups-grid">
            {groupItems.map((item, index) => {
              if (item.expanded) {
                return (
                  <div key={item.group.id} className="group-expand-band">
                    <button
                      type="button"
                      className={`tile group-collapse-tile${
                        dropKey === `group:${item.group.id}` ? " drop-target" : ""
                      }`}
                      style={
                        {
                          animationDelay: `${Math.min(index, 12) * 0.02}s`,
                          "--group-accent": item.accent,
                        } as React.CSSProperties
                      }
                      data-drop-group={item.group.id}
                      onClick={() => onExpandedGroupChange(null)}
                      title="Collapse group"
                    >
                      <div className="cover group-collapse-cover">
                        <span className="group-collapse-label">{item.group.name}</span>
                        <span className="group-collapse-meta">
                          {item.members.length} · click to stack
                        </span>
                      </div>
                      <div className="tile-title">{item.group.name}</div>
                      <div className="tile-store">Collapse</div>
                    </button>
                    {item.members.map((game, i) => (
                      <GameTile
                        key={`${item.group.id}:${game.id}`}
                        game={game}
                        index={i}
                        coverDataUrl={coverMap[game.id]}
                        groups={groups}
                        inGroup={item.group}
                        groupAccent={item.accent}
                        expandIndex={i}
                        onOpen={onOpen}
                        onLaunch={onLaunch}
                        onToggleFavorite={onToggleFavorite}
                        onHide={onHide}
                        onOpenFolder={onOpenFolder}
                        onAddToGroup={onAddToGroup}
                        onRemoveFromGroup={() => onRemoveFromGroup(item.group, game)}
                      />
                    ))}
                  </div>
                );
              }

              return (
                <GroupTile
                  key={item.group.id}
                  group={item.group}
                  members={item.members}
                  coverMap={coverMap}
                  index={index}
                  dropActive={dropKey === `group:${item.group.id}`}
                  dragActive={activeKey === `group:${item.group.id}`}
                  onPointerDragStart={(e, g) =>
                    beginPointerSession(e, { type: "group", groupId: g.id })
                  }
                  onOpen={() => onExpandedGroupChange(item.group.id)}
                  onRename={onRenameGroup}
                  onDelete={onDeleteGroup}
                  onAddGames={() => onAddGamesToGroup(item.group)}
                />
              );
            })}
          </div>
        </section>
      )}

      <section className="library-section games-section">
        {groupItems.length > 0 && <h3 className="section-label">Games</h3>}
        <div className="game-grid" ref={gamesGridRef}>
          {ungroupedGames.map((game, index) => {
            const layoutKey = gameOrderKey(game.id);
            return (
              <GameTile
                key={game.id}
                game={game}
                index={index}
                coverDataUrl={coverMap[game.id]}
                groups={groups}
                dropActive={dropKey === layoutKey}
                dragActive={activeKey === layoutKey}
                onPointerDragStart={(e, g) =>
                  beginPointerSession(e, { type: "game", gameId: g.id })
                }
                onOpen={onOpen}
                onLaunch={onLaunch}
                onToggleFavorite={onToggleFavorite}
                onHide={onHide}
                onOpenFolder={onOpenFolder}
                onAddToGroup={onAddToGroup}
              />
            );
          })}
          {Array.from({ length: ghosts }, (_, i) => (
            <button
              key={`ghost-${i}`}
              type="button"
              className={`tile-ghost${dropKey === "end" && i === ghosts - 1 ? " drop-target" : ""}`}
              onClick={onAddGame}
              aria-label="Add game"
              title="Add game"
              data-drop-end={i === ghosts - 1 ? "1" : undefined}
            >
              +
            </button>
          ))}
        </div>
      </section>

      {preview && (
        <div
          className="drag-float"
          style={{
            width: preview.width,
            transform: `translate3d(${preview.x - preview.offsetX}px, ${preview.y - preview.offsetY}px, 0)`,
          }}
          aria-hidden
        >
          <div className={`cover${preview.stacked ? " group-stack" : ""}`}>
            {preview.stacked && preview.stacked.length > 0 ? (
              <div className="stack-layers">
                {preview.stacked.map((layer, i) => (
                  <div
                    key={i}
                    className={`stack-card stack-card-${i}`}
                    style={{ zIndex: preview.stacked!.length - i }}
                  >
                    {layer.cover ? (
                      <img src={layer.cover} alt="" draggable={false} />
                    ) : (
                      <div className="cover-fallback">{layer.initial}</div>
                    )}
                  </div>
                ))}
              </div>
            ) : preview.cover ? (
              <img src={preview.cover} alt="" draggable={false} />
            ) : (
              <div className="cover-fallback">{preview.initial}</div>
            )}
          </div>
          <div className="tile-title">{preview.title}</div>
        </div>
      )}
    </div>
  );
}
