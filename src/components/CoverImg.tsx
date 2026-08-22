import { useEffect, useMemo, useRef, useState } from "react";
import { Game, coverCandidates } from "../types";

interface Props {
  game: Game;
  override?: string | null;
  className?: string;
  fallbackClassName?: string;
  loading?: "lazy" | "eager";
  draggable?: boolean;
  skipShapeCheck?: boolean;
  /** Grid tiles stay local-only so Steam 600×900 never decodes in bulk. */
  allowRemote?: boolean;
}

function isLandscapeCover(img: HTMLImageElement): boolean {
  const w = img.naturalWidth;
  const h = img.naturalHeight;
  return w > 16 && h > 16 && h < w * 0.85;
}

export function CoverImg({
  game,
  override,
  className,
  fallbackClassName = "cover-fallback",
  loading = "lazy",
  draggable = false,
  skipShapeCheck = false,
  allowRemote = true,
}: Props) {
  const defer = loading !== "eager";
  const wrapRef = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(!defer);
  const candidates = useMemo(
    () => coverCandidates(game, override, { allowRemote }),
    [
      game.id,
      game.coverPath,
      game.steamAppId,
      game.coverUrl,
      game.store,
      game.launchTarget,
      override,
      allowRemote,
    ],
  );
  const [index, setIndex] = useState(0);
  const [exhausted, setExhausted] = useState(false);
  const indexRef = useRef(0);
  const imgRef = useRef<HTMLImageElement | null>(null);
  indexRef.current = index;

  useEffect(() => {
    if (!defer) {
      setVisible(true);
      return;
    }
    const el = wrapRef.current;
    if (!el) return;
    const root = el.closest(".main") ?? document.querySelector(".main");
    const io = new IntersectionObserver(([entry]) => {
      if (
        !entry.isIntersecting &&
        wrapRef.current?.closest("[data-library-hidden], [hidden]")
      ) {
        return;
      }
      setVisible(entry.isIntersecting);
    }, {
      root: root instanceof Element ? root : null,
      rootMargin: "80px 0px",
    });
    io.observe(el);
    return () => io.disconnect();
  }, [defer, game.id]);

  useEffect(() => {
    setIndex(0);
    indexRef.current = 0;
    setExhausted(false);
  }, [game.id, game.coverPath, override]);

  const src = candidates[index];
  const initial = game.name.trim().charAt(0).toUpperCase() || "?";

  useEffect(() => {
    if (!visible) return;
    const img = imgRef.current;
    if (!img || skipShapeCheck) return;
    if (img.complete && img.naturalWidth > 0 && isLandscapeCover(img)) {
      const next = indexRef.current + 1;
      if (next < candidates.length) setIndex(next);
      else setExhausted(true);
    }
  }, [src, skipShapeCheck, candidates.length, visible]);

  const fallback = <div className={fallbackClassName}>{initial}</div>;
  const body =
    !visible || !src || exhausted ? (
      fallback
    ) : (
      <img
        ref={imgRef}
        src={src}
        alt=""
        className={className}
        loading={loading}
        decoding="async"
        fetchPriority="low"
        draggable={draggable}
        onLoad={(e) => {
          if (skipShapeCheck) return;
          if (!isLandscapeCover(e.currentTarget)) return;
          const next = indexRef.current + 1;
          if (next < candidates.length) setIndex(next);
          else setExhausted(true);
        }}
        onError={() => {
          const next = indexRef.current + 1;
          if (next < candidates.length) setIndex(next);
          else setExhausted(true);
        }}
      />
    );

  return (
    <span ref={wrapRef} className="cover-img-host">
      {body}
    </span>
  );
}
