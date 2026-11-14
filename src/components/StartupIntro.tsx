import { useEffect, useState } from "react";
import type { ThemeId } from "../types";
import "../styles/StartupIntro.css";

export function StartupIntro({ theme, onFinish }: { theme: ThemeId | "pending"; onFinish: () => void }) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (motion.matches) onFinish();
    const onMotionChange = () => { if (motion.matches) onFinish(); };
    motion.addEventListener("change", onMotionChange);
    // A stalled settings read, image load or animation must still yield to the launcher.
    const timeout = window.setTimeout(onFinish, ready ? 2800 : 1500);
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
      onAnimationEnd={onFinish}
    >
      {theme !== "pending" && (
        <img src={`/startup/${theme}.webp`} width="320" height="200" alt="" draggable={false}
          onLoad={() => setReady(true)} onError={onFinish} />
      )}
    </button>
  );
}
