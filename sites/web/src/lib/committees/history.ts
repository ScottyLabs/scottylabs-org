// Undo and redo for the page editor. Each entry is a whole page, which is small. Changes with the same key in quick succession, such as typing in one field, share one entry.

export const HISTORY_LIMIT = 50;
const MERGE_WINDOW_MS = 1000;

export class History<T> {
  #past: T[] = [];
  #future: T[] = [];
  #lastKey: string | null = null;
  #lastAt = 0;
  readonly #limit: number;

  constructor(limit = HISTORY_LIMIT) {
    this.#limit = limit;
  }

  get canUndo(): boolean {
    return this.#past.length > 0;
  }

  get canRedo(): boolean {
    return this.#future.length > 0;
  }

  /**
   * Records the state before a change. With a `key`, a change right after another with the same
   * key adds nothing, so undo goes back to before the whole burst.
   */
  record(before: T, key: string | null = null, now = Date.now()): void {
    const merges = key !== null && key === this.#lastKey && now - this.#lastAt < MERGE_WINDOW_MS;
    this.#lastKey = key;
    this.#lastAt = now;
    if (merges) return;
    this.#past.push(before);
    if (this.#past.length > this.#limit) this.#past.shift();
    this.#future = [];
  }

  /** The state to go back to, given the current one, or null when there is nothing to undo. */
  undo(current: T): T | null {
    const previous = this.#past.pop();
    if (previous === undefined) return null;
    this.#future.push(current);
    this.#lastKey = null;
    return previous;
  }

  redo(current: T): T | null {
    const next = this.#future.pop();
    if (next === undefined) return null;
    this.#past.push(current);
    this.#lastKey = null;
    return next;
  }

  /** Ends the current burst, so the next change gets its own entry. */
  seal(): void {
    this.#lastKey = null;
  }

  clear(): void {
    this.#past = [];
    this.#future = [];
    this.#lastKey = null;
  }
}
