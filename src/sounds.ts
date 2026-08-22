const MUTE_KEY = "intelgen.bpMute";

let ctx: AudioContext | null = null;
let muted = false;

try {
  muted = localStorage.getItem(MUTE_KEY) === "1";
} catch {
  muted = false;
}

function ac(): AudioContext | null {
  if (typeof window === "undefined") return null;
  const AC = window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!AC) return null;
  if (!ctx) ctx = new AC();
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

function envGain(c: AudioContext, start: number, peak: number, dur: number) {
  const g = c.createGain();
  g.gain.setValueAtTime(0.0001, start);
  g.gain.exponentialRampToValueAtTime(Math.max(0.0002, peak), start + 0.012);
  g.gain.exponentialRampToValueAtTime(0.0001, start + dur);
  g.connect(c.destination);
  return g;
}

function beep(
  freq: number,
  dur: number,
  type: OscillatorType,
  peak: number,
  slide?: number,
  delay = 0,
) {
  const c = ac();
  if (!c || muted) return;
  const t = c.currentTime + delay;
  const o = c.createOscillator();
  o.type = type;
  o.frequency.setValueAtTime(freq, t);
  if (slide) o.frequency.exponentialRampToValueAtTime(Math.max(20, slide), t + dur);
  o.connect(envGain(c, t, peak, dur));
  o.start(t);
  o.stop(t + dur + 0.02);
}

export function isBpMuted(): boolean {
  return muted;
}

export function setBpMuted(next: boolean) {
  muted = next;
  try {
    localStorage.setItem(MUTE_KEY, next ? "1" : "0");
  } catch {
    /* ignore */
  }
  if (!next) ac();
}

export function playMove() {
  beep(920, 0.045, "sine", 0.035, 1180);
}

export function playConfirm() {
  beep(523.25, 0.07, "triangle", 0.07);
  beep(783.99, 0.11, "sine", 0.055, undefined, 0.06);
}

export function playBack() {
  beep(392, 0.07, "sine", 0.05, 262);
}

export function playError() {
  beep(160, 0.16, "square", 0.045, 110);
}

export function playBoot() {
  beep(110, 0.55, "triangle", 0.05, 220);
  beep(220, 0.4, "sine", 0.04, 440, 0.18);
  beep(330, 0.55, "sine", 0.035, 660, 0.42);
}

export function playReady() {
  beep(523.25, 0.1, "sine", 0.05);
  beep(659.25, 0.12, "sine", 0.045, undefined, 0.09);
  beep(783.99, 0.18, "triangle", 0.055, undefined, 0.18);
}

export function playPower() {
  beep(180, 0.28, "triangle", 0.06, 70);
}
