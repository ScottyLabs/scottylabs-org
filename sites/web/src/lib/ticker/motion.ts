// Browser pieces of the ticker's motion: the remembered pause button and the moving track.

const PAUSE_KEY = "sl-ticker-paused";

/** Whether the visitor paused the ticker earlier in this visit. Storage can be blocked, so this never throws. */
export function readPaused(): boolean {
  try {
    return sessionStorage.getItem(PAUSE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writePaused(paused: boolean): void {
  try {
    if (paused) sessionStorage.setItem(PAUSE_KEY, "1");
    else sessionStorage.removeItem(PAUSE_KEY);
  } catch {
    // Blocked storage only means the choice is not remembered.
  }
}

/** Pixels per second the ticker moves. */
export const TICKER_SPEED = 40;

/**
 * Moves a track of identical sets from right to left with the Web Animations API, so the browser
 * animates a transform off the main thread. One loop covers two sets: the track holds a copy
 * before the real set, so any item of the real set can be brought into view with content on
 * both sides of it.
 */
export class Marquee {
  readonly #track: HTMLElement;
  readonly #speed: number;
  #animation: Animation | null = null;
  #span = 0;
  #playing = false;

  constructor(track: HTMLElement, speed = TICKER_SPEED) {
    this.#track = track;
    this.#speed = speed;
  }

  /** Starts or resizes the loop for sets this wide, keeping the current position. */
  measure(setWidth: number): void {
    const span = setWidth * 2;
    if (span <= 0 || (Math.abs(span - this.#span) < 0.5 && this.#animation !== null)) return;
    const offset = this.#offset();
    this.#animation?.cancel();
    this.#span = span;
    this.#animation = this.#track.animate(
      [{ transform: "translateX(0)" }, { transform: `translateX(${-span}px)` }],
      { duration: (span / this.#speed) * 1000, iterations: Infinity, easing: "linear" },
    );
    this.#seek(offset);
    if (!this.#playing) this.#animation.pause();
  }

  play(): void {
    this.#playing = true;
    this.#animation?.play();
  }

  pause(): void {
    this.#playing = false;
    this.#animation?.pause();
  }

  /** Moves the track so the point `left` pixels into it sits `inset` pixels from the window's left edge. */
  reveal(left: number, inset: number): void {
    this.#seek(Math.min(Math.max(left - inset, 0), this.#span));
  }

  destroy(): void {
    this.#animation?.cancel();
    this.#animation = null;
  }

  #offset(): number {
    const time = this.#animation?.currentTime;
    if (typeof time !== "number" || this.#span === 0) return 0;
    return ((time / 1000) * this.#speed) % this.#span;
  }

  #seek(offset: number): void {
    if (this.#animation === null) return;
    this.#animation.currentTime = (offset / this.#speed) * 1000;
  }
}
