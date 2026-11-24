// How the ticker shows its announcements. Plain logic, so it can be tested without a browser.

export type TickerMode = "loading" | "empty" | "static" | "stepper" | "marquee";

export interface ModeInput {
  /** How many announcements are live, or null while they load. */
  count: number | null;
  reducedMotion: boolean;
  /** The only announcement is wider than the strip, so it scrolls to be read in full. */
  tooWide: boolean;
}

/**
 * One announcement stands still when it fits; otherwise the strip scrolls. With reduced motion
 * nothing scrolls: several announcements are shown one at a time.
 */
export function tickerMode({ count, reducedMotion, tooWide }: ModeInput): TickerMode {
  if (count === null) return "loading";
  if (count === 0) return "empty";
  if (reducedMotion) return count === 1 ? "static" : "stepper";
  return count === 1 && !tooWide ? "static" : "marquee";
}

/**
 * Whether the strip offers the list of every announcement. Where an announcement stands still and
 * is cut short, the list is the only place to read all of it.
 */
export function offersList(mode: TickerMode, cutShort: boolean): boolean {
  return mode === "marquee" || mode === "stepper" || (mode === "static" && cutShort);
}

/** Whether the strip should hold its height while loading, given the size of the last answer. */
export function reservesSpace(lastCount: number | null): boolean {
  return lastCount !== 0;
}
