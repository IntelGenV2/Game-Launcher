import { ReactNode, useEffect, useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";

export type MenuAnchor = { x: number; y: number; align?: "left" | "right" };

interface Props {
  open: boolean;
  anchor: MenuAnchor | null;
  onClose: () => void;
  children: ReactNode;
}

export function TileContextMenu({ open, anchor, onClose, children }: Props) {
  const menuRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const el = menuRef.current;
    if (!open || !el || !anchor) return;
    const pad = 8;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let top = anchor.y;
    let left = anchor.align === "right" ? anchor.x - w : anchor.x;
    if (top + h > window.innerHeight - pad) top = Math.max(pad, anchor.y - h);
    if (left + w > window.innerWidth - pad) left = window.innerWidth - w - pad;
    if (left < pad) left = pad;
    if (top < pad) top = pad;
    el.style.top = `${top}px`;
    el.style.left = `${left}px`;
  }, [open, anchor, children]);

  useEffect(() => {
    if (!open) return;

    let eatUntilClick = false;
    const eat = (e: Event) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.type === "click") {
        document.removeEventListener("pointerup", eat, true);
        document.removeEventListener("click", eat, true);
        eatUntilClick = false;
      }
    };

    const onPointerDown = (e: PointerEvent) => {
      if (menuRef.current?.contains(e.target as Node)) return;
      e.preventDefault();
      e.stopPropagation();
      if (!eatUntilClick) {
        eatUntilClick = true;
        document.addEventListener("pointerup", eat, true);
        document.addEventListener("click", eat, true);
      }
      onClose();
    };

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onScroll = () => onClose();
    const onContextMenu = (e: Event) => {
      if (menuRef.current?.contains(e.target as Node)) return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
    };

    document.addEventListener("pointerdown", onPointerDown, true);
    document.addEventListener("keydown", onKey);
    document.addEventListener("contextmenu", onContextMenu, true);
    document.querySelector(".main")?.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onClose);

    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("contextmenu", onContextMenu, true);
      document.removeEventListener("pointerup", eat, true);
      document.removeEventListener("click", eat, true);
      document.querySelector(".main")?.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onClose);
    };
  }, [open, onClose]);

  if (!open || !anchor) return null;

  return createPortal(
    <>
      <div className="menu-dismiss" />
      <div
        ref={menuRef}
        className="context-menu context-menu-portal"
        role="menu"
        style={{ top: anchor.y, left: anchor.x }}
        onClick={(e) => e.stopPropagation()}
        onPointerDown={(e) => e.stopPropagation()}
      >
        {children}
      </div>
    </>,
    document.body,
  );
}

export function anchorFromElement(el: HTMLElement | null, align: "left" | "right" = "right"): MenuAnchor | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: r.right, y: r.bottom + 6, align };
}

export function anchorFromPoint(x: number, y: number): MenuAnchor {
  return { x, y, align: "left" };
}
