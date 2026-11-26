// The last public content a visitor loaded, kept in their browser. When a public page cannot load its content, it shows this copy with a note (LastGoodNote.svelte) instead of an error. Copies are the JSON the service sent, checked for shape before use and dropped after a week. Storage can be blocked or full, so nothing here throws.

const PREFIX = "sl-last-good:";
/** Copies older than this are not shown: the published content may have changed too much. */
export const MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000;
/** Larger answers are not kept, so one page cannot fill the visitor's storage. */
const MAX_CHARS = 512 * 1024;

export interface SavedCopy<T> {
  value: T;
  /** When it was loaded, in milliseconds since the epoch. */
  savedAt: number;
}

/** Where copies live: localStorage in the browser. Tests pass their own. */
export interface CopyStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function browserStore(): CopyStore | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Keeps `value` as the last good copy under `key`. */
export function remember(
  key: string,
  value: unknown,
  store = browserStore(),
  now = Date.now(),
): void {
  if (store === null) return;
  try {
    const text = JSON.stringify({ value, savedAt: now });
    if (text.length > MAX_CHARS) store.removeItem(PREFIX + key);
    else store.setItem(PREFIX + key, text);
  } catch {
    // Full or blocked storage only means no copy is kept.
  }
}

/** The copy under `key`, if there is one of the right shape and less than a week old. */
export function recall<T>(
  key: string,
  isShape: (value: unknown) => value is T,
  store = browserStore(),
  now = Date.now(),
): SavedCopy<T> | null {
  if (store === null) return null;
  try {
    const text = store.getItem(PREFIX + key);
    if (text === null) return null;
    const saved: unknown = JSON.parse(text);
    if (!isRecord(saved)) return null;
    const { value, savedAt } = saved;
    const fresh = typeof savedAt === "number" && savedAt <= now && now - savedAt <= MAX_AGE_MS;
    if (!fresh) {
      store.removeItem(PREFIX + key);
      return null;
    }
    return isShape(value) ? { value, savedAt } : null;
  } catch {
    return null;
  }
}

/** Drops the copy under `key`, for content that is no longer published. */
export function forget(key: string, store = browserStore()): void {
  try {
    store?.removeItem(PREFIX + key);
  } catch {
    // Blocked storage holds no copy to drop.
  }
}

/**
 * The note above content shown from a saved copy. `savedAt` is null when the page keeps what it
 * loaded earlier in this visit.
 */
export function lastGoodMessage(savedAt: number | null, now = Date.now()): string {
  if (savedAt === null)
    return "We couldn\u2019t refresh this page, so you\u2019re seeing what loaded before.";
  const saved = new Date(savedAt);
  const today = new Date(now);
  const sameDay = saved.toDateString() === today.toDateString();
  const when = sameDay
    ? "earlier today"
    : `on ${saved.toLocaleDateString("en-US", { month: "long", day: "numeric" })}`;
  return `We couldn\u2019t reach the site, so this is how the page looked when you visited ${when}.`;
}
