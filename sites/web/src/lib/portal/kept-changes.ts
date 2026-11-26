// Unsaved work kept in this tab when the session ends while someone is editing (spec S-5). Editors keep it from PortalSession.onEnd, before the portal sends the person to sign in and back to the same page, which puts the work back. Kept work lives in sessionStorage: it belongs to this tab, is gone when the tab closes, and is only handed back to the person who made it. Signing out clears it, and someone else signing in on the tab clears what is not theirs. Plain logic, so it can be tested without a browser.

const PREFIX = "sl:kept-changes:";

/** Older work is dropped instead of being put back over whatever happened since. */
export const KEPT_FOR_MS = 24 * 60 * 60 * 1000;

export interface KeptChanges<T> {
  /** Who made the changes. Nobody else gets them back. */
  userId: string;
  /**
   * What the changes were made on top of, such as the draft's version or update time. They go
   * back on their own only while the record is still at this point.
   */
  base: string;
  value: T;
  /** When they were kept, in milliseconds since the epoch. */
  keptAt: number;
}

/** The parts of Storage used here, so tests can pass a stand-in. */
export type KeptStore = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** What clearing needs on top: a way to list what is kept. */
export type KeptListStore = KeptStore & Pick<Storage, "key" | "length">;

function tabStore(): Storage | null {
  try {
    return globalThis.sessionStorage;
  } catch {
    // Storage blocked by the browser's settings: nothing can be kept.
    return null;
  }
}

/** Keeps unsaved work under `key`, replacing anything kept there before. */
export function keepChanges<T>(
  key: string,
  changes: Omit<KeptChanges<T>, "keptAt">,
  now = Date.now(),
  store: KeptStore | null = tabStore(),
): void {
  const kept: KeptChanges<T> = { ...changes, keptAt: now };
  try {
    store?.setItem(PREFIX + key, JSON.stringify(kept));
  } catch {
    // Storage full or blocked: the work cannot be kept, as before.
  }
}

/**
 * The work kept under `key` for `userId`, once it passes `isValue`. Work kept by someone else, kept
 * too long ago, or no longer readable is dropped and gives null.
 */
export function keptChanges<T>(
  key: string,
  userId: string,
  isValue: (value: unknown) => value is T,
  now = Date.now(),
  store: KeptStore | null = tabStore(),
): KeptChanges<T> | null {
  let raw: string | null;
  try {
    raw = store?.getItem(PREFIX + key) ?? null;
  } catch {
    return null;
  }
  if (raw === null) return null;
  const kept = parseKept(raw, isValue);
  const fresh = kept !== null && kept.keptAt <= now && now - kept.keptAt <= KEPT_FOR_MS;
  if (kept === null || !fresh || kept.userId !== userId) {
    forgetChanges(key, store);
    return null;
  }
  return kept;
}

export function forgetChanges(key: string, store: KeptStore | null = tabStore()): void {
  try {
    store?.removeItem(PREFIX + key);
  } catch {
    // Nothing to forget when storage is blocked.
  }
}

/**
 * Removes the work kept in this tab, all of it or all but `keepFor`'s. Signing out clears all of
 * it, and when a session ends or someone signs in, only that person's work stays (PortalSession),
 * so one person's unsaved work never waits in the tab for whoever uses it next.
 */
export function clearKeptChanges(
  keepFor: string | null = null,
  store: KeptListStore | null = tabStore(),
): void {
  if (store === null) return;
  try {
    const keys: string[] = [];
    for (let index = 0; index < store.length; index += 1) {
      const key = store.key(index);
      if (key?.startsWith(PREFIX) === true) keys.push(key);
    }
    for (const key of keys) {
      if (keepFor !== null && keptBy(store.getItem(key)) === keepFor) continue;
      store.removeItem(key);
    }
  } catch {
    // Storage blocked by the browser's settings: nothing was kept.
  }
}

/** Who kept the work stored as `raw`, or null when it cannot be read. */
function keptBy(raw: string | null): string | null {
  if (raw === null) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return isRecord(parsed) && typeof parsed.userId === "string" ? parsed.userId : null;
  } catch {
    return null;
  }
}

/**
 * What to do with kept work when the record has loaded: nothing when there is none or it matches
 * what is there now, put it back when the record has not changed since it was kept, and otherwise
 * ask, because putting it back would replace someone else's newer changes.
 */
export function restorePlan<T>(
  kept: KeptChanges<T> | null,
  current: { base: string; value: T },
): "none" | "restore" | "ask" {
  if (kept === null || JSON.stringify(kept.value) === JSON.stringify(current.value)) return "none";
  return kept.base === current.base ? "restore" : "ask";
}

/** True for a plain object; a start for the value checks callers pass to `keptChanges`. */
export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseKept<T>(raw: string, isValue: (value: unknown) => value is T): KeptChanges<T> | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (
    !isRecord(parsed) ||
    typeof parsed.userId !== "string" ||
    typeof parsed.base !== "string" ||
    typeof parsed.keptAt !== "number" ||
    !isValue(parsed.value)
  ) {
    return null;
  }
  return { userId: parsed.userId, base: parsed.base, value: parsed.value, keptAt: parsed.keptAt };
}
