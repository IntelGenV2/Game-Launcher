import { useEffect, useRef, useState } from "react";

export type PadAction =
  | "up"
  | "down"
  | "left"
  | "right"
  | "confirm"
  | "back"
  | "favorite"
  | "select";

const REPEAT_MS = 180;
const DEADZONE = 0.55;

/**
 * Lightweight Gamepad API poller. Fires discrete actions with repeat for D-pad / stick.
 * Returns whether a gamepad is currently connected.
 */
export function useGamepad(onAction: (action: PadAction) => void, enabled = true): boolean {
  const [connected, setConnected] = useState(false);
  const onActionRef = useRef(onAction);
  onActionRef.current = onAction;
  const prevRef = useRef<Record<string, boolean>>({});
  const lastFireRef = useRef<Record<string, number>>({});

  useEffect(() => {
    if (!enabled) return;

    let raf = 0;
    const tick = () => {
      const pads = navigator.getGamepads?.() ?? [];
      let any = false;
      for (const pad of pads) {
        if (!pad) continue;
        any = true;
        const now = performance.now();

        const fire = (name: string, action: PadAction, pressed: boolean) => {
          const key = `${pad.index}:${name}`;
          const was = prevRef.current[key] ?? false;
          if (pressed && !was) {
            onActionRef.current(action);
            lastFireRef.current[key] = now;
          } else if (pressed && was) {
            const last = lastFireRef.current[key] ?? 0;
            if (now - last >= REPEAT_MS) {
              onActionRef.current(action);
              lastFireRef.current[key] = now;
            }
          }
          prevRef.current[key] = pressed;
        };

        const b = pad.buttons;
        const ax = pad.axes;
        const left = (b[14]?.pressed ?? false) || (ax[0] ?? 0) < -DEADZONE;
        const right = (b[15]?.pressed ?? false) || (ax[0] ?? 0) > DEADZONE;
        const up = (b[12]?.pressed ?? false) || (ax[1] ?? 0) < -DEADZONE;
        const down = (b[13]?.pressed ?? false) || (ax[1] ?? 0) > DEADZONE;

        fire("left", "left", left);
        fire("right", "right", right);
        fire("up", "up", up);
        fire("down", "down", down);
        // A / Cross
        fire("a", "confirm", b[0]?.pressed ?? false);
        // B / Circle
        fire("b", "back", b[1]?.pressed ?? false);
        // X / Square
        fire("x", "favorite", b[2]?.pressed ?? false);
        // View / Select / Back button
        fire("select", "select", b[8]?.pressed ?? false);
      }
      setConnected(any);
      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);
    const onConnect = () => setConnected(true);
    const onDisconnect = () => {
      const pads = navigator.getGamepads?.() ?? [];
      setConnected(pads.some(Boolean));
    };
    window.addEventListener("gamepadconnected", onConnect);
    window.addEventListener("gamepaddisconnected", onDisconnect);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("gamepadconnected", onConnect);
      window.removeEventListener("gamepaddisconnected", onDisconnect);
    };
  }, [enabled]);

  return connected;
}

export function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    el.isContentEditable
  );
}
