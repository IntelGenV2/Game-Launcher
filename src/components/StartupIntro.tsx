import { useEffect, useState } from "react";
import type { ThemeId } from "../types";
import "../styles/StartupIntro.css";

function StartupWordmark() {
  return (
    <svg className="startup-wordmark" viewBox="0 0 303 50" aria-hidden="true">
      <defs>
        <g id="startup-studio" fillRule="evenodd">
          <path d="M6 0h20v7H9L7 9v3l2 2h12l5 5v10l-5 5H0v-7h17l2-2v-3l-2-2H5l-5-5V6z" />
          <path transform="translate(31)" d="M0 0h27v7H17v27h-7V7H0z" />
          <path transform="translate(63)" d="M0 0h7v24l3 3h7l3-3V0h7v28l-6 6H6l-6-6z" />
          <path transform="translate(96)" d="M0 0h19l8 8v18l-8 8H0z M7 7v20h9l4-4V11l-4-4z" />
          <path transform="translate(129)" d="M0 0h9v34H0z" />
          <path transform="translate(144)" d="M6 0h15l6 6v22l-6 6H6l-6-6V6z M9 7l-2 2v16l2 2h9l2-2V9l-2-2z" />
        </g>
        <g id="startup-256" fillRule="evenodd">
          <path d="M6 0h15l6 6v11l-5 5-13 3-2 2h20v7H0V24l6-6 13-3 1-2V9l-2-2H9L7 9v2H0V6z" />
          <path transform="translate(33)" d="M1 0h25v7H8v6h13l6 6v9l-6 6H5l-5-5v-5h7v1l2 2h9l2-2v-3l-2-2H1z" />
          <path transform="translate(66)" d="M6 0h20v7H10l-3 3v3h14l6 6v9l-6 6H6l-6-6V6z M7 20v5l2 2h9l2-2v-3l-2-2z" />
        </g>
      </defs>
      {["startup-studio", "startup-256"].map((id, word) => (
        <g key={id} transform={`translate(${word ? 195 : 8} 4) skewX(-5)`}>
          <g className={`startup-word startup-word-${word ? "number" : "name"}`}>
            {[5, 4, 3, 2, 1].map(depth => (
              <use key={depth} href={`#${id}`} x={depth} y={depth} className="startup-word-edge" />
            ))}
            <use href={`#${id}`} />
          </g>
        </g>
      ))}
    </svg>
  );
}

export function StartupIntro({ theme, onFinish }: { theme: ThemeId | "pending"; onFinish: () => void }) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (motion.matches) onFinish();
    const onMotionChange = () => { if (motion.matches) onFinish(); };
    motion.addEventListener("change", onMotionChange);
    // A stalled settings read, image load or animation must still yield to the launcher.
    const timeout = window.setTimeout(onFinish, ready ? 3500 : 1500);
    return () => {
      window.clearTimeout(timeout);
      motion.removeEventListener("change", onMotionChange);
    };
  }, [theme, ready, onFinish]);

  return (
    <button
      type="button"
      className={`startup-intro${ready ? " is-playing" : ""}`}
      aria-label="Skip Studio 256 opening animation"
      onClick={onFinish}
      onAnimationEnd={event => { if (event.target === event.currentTarget) onFinish(); }}
    >
      {theme !== "pending" && (
        <span className="startup-art">
          <img src={`/startup/${theme}.webp`} width="320" height="200" alt="" draggable={false}
            onLoad={() => setReady(true)} onError={onFinish} />
          <StartupWordmark />
        </span>
      )}
    </button>
  );
}
