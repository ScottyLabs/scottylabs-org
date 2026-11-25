// When the editor asks for the edit lock (spec F7). The service keeps a lock for five minutes after the last request or save, and the editor asks again only while the person is working, so a page left open in a background tab frees up five to six minutes after they last did anything.

/** How long the service keeps a lock without a request. Mirrors `LOCK_MINUTES` in the service. */
export const LOCK_MS = 5 * 60_000;
/** How often the editor asks to keep the lock while the person is working. */
export const HEARTBEAT_MS = 60_000;

export class LockClock {
  #activeAt: number;
  #claimedAt: number | null = null;

  constructor(now: number) {
    this.#activeAt = now;
  }

  /**
   * Records that the person did something in the editor. Returns true when the lock has lapsed
   * since the last request, so the editor should ask for it again right away.
   */
  touch(now: number): boolean {
    this.#activeAt = now;
    return this.#claimedAt !== null && now - this.#claimedAt >= LOCK_MS;
  }

  /** Records a request for the lock, or a save, which refreshes it too. */
  claimed(now: number): void {
    this.#claimedAt = now;
  }

  /** On each heartbeat: ask again only when the person did something since the last request. */
  get due(): boolean {
    return this.#claimedAt === null || this.#activeAt > this.#claimedAt;
  }
}
