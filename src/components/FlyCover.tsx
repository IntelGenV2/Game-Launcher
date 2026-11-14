import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Game } from "../types";
import { CoverImg } from "./CoverImg";

export interface FlyOrigin {
  from: DOMRect;
  src: string | null;
  name: string;
  game: Game;
}

export function FlyCover({ fly, onDone }: { fly: FlyOrigin; onDone: () => void }) {
  const [to, setTo] = useState<DOMRect | null>(null);
  const [on, setOn] = useState(false);
  const doneRef = useRef(onDone);
  doneRef.current = onDone;
  const finished = useRef(false);

  useLayoutEffect(() => {
    finished.current = false;
    const finish = () => {
      if (finished.current) return;
      finished.current = true;
      doneRef.current();
    };

    setOn(false);
    // Measure after the detail page has reset its scroll position and laid out.
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        const el = document.querySelector(".detail-cover");
        if (!el) { finish(); return; }
        setTo(el.getBoundingClientRect());
        frame = requestAnimationFrame(() => setOn(true));
      });
    });
    const timeout = window.setTimeout(finish, 800);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timeout);
    };
  }, [fly.game.id]);

  if (!to) return null;
  const dx = to.left - fly.from.left;
  const dy = to.top - fly.from.top;
  const sx = fly.from.width ? to.width / fly.from.width : 1;
  const sy = fly.from.height ? to.height / fly.from.height : 1;

  return createPortal(
    <div
      className={`fly-cover${on ? " fly-on" : ""}`}
      style={{
        left: fly.from.left,
        top: fly.from.top,
        width: fly.from.width,
        height: fly.from.height,
        transform: on ? `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})` : "none",
      }}
      onTransitionEnd={(e) => {
        if (e.propertyName === "transform" && !finished.current) {
          finished.current = true;
          doneRef.current();
        }
      }}
    >
      <CoverImg game={fly.game} override={fly.src} loading="eager" skipShapeCheck />
    </div>, document.body
  );
}
