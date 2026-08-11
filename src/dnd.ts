/** Pointer-based library drag — HTML5 DnD fights Tauri/WebView2 file-drop (red ∅ cursor). */

export type LibraryDrag =
  | { type: "game"; gameId: string }
  | { type: "group"; groupId: string };

export type LibraryDropTarget =
  | { kind: "game"; gameId: string }
  | { kind: "group"; groupId: string }
  | { kind: "end" }
  | null;

const THRESHOLD_PX = 6;

export function libraryDragKey(drag: LibraryDrag): string {
  return drag.type === "game" ? `game:${drag.gameId}` : `group:${drag.groupId}`;
}

export function dropTargetKey(target: LibraryDropTarget): string | null {
  if (!target) return null;
  if (target.kind === "game") return `game:${target.gameId}`;
  if (target.kind === "group") return `group:${target.groupId}`;
  return "end";
}

export function resolveDropTarget(
  clientX: number,
  clientY: number,
  drag: LibraryDrag | null,
): LibraryDropTarget {
  const el = document.elementFromPoint(clientX, clientY) as HTMLElement | null;
  if (!el) return null;

  const groupHost = el.closest<HTMLElement>("[data-drop-group]");
  if (groupHost) {
    const groupId = groupHost.dataset.dropGroup;
    if (groupId && !(drag?.type === "group" && drag.groupId === groupId)) {
      return { kind: "group", groupId };
    }
  }

  const gameHost = el.closest<HTMLElement>("[data-drop-game]");
  if (gameHost) {
    const gameId = gameHost.dataset.dropGame;
    if (gameId && !(drag?.type === "game" && drag.gameId === gameId)) {
      return { kind: "game", gameId };
    }
  }

  if (el.closest("[data-drop-end]")) return { kind: "end" };
  return null;
}

export type PointerDragSession = {
  drag: LibraryDrag;
  active: boolean;
  startX: number;
  startY: number;
  pointerId: number;
  offsetX: number;
  offsetY: number;
  width: number;
};

export function shouldActivateDrag(session: PointerDragSession, x: number, y: number): boolean {
  if (session.active) return false;
  return Math.hypot(x - session.startX, y - session.startY) >= THRESHOLD_PX;
}
