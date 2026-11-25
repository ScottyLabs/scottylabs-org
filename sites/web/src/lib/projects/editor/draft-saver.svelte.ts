// Saves a project's draft a moment after the last change (spec PR-7, CP-7). One save runs at a time; changes made while it runs are saved right after. Field problems and lasting failures (no access, gone, too large) stop saving until the next change; dropped connections and server trouble retry on their own.

import { isApiError } from "../../api/client.ts";
import type { ProjectDoc, ProjectEditorResponse } from "../../api/types.ts";
import { retriesOnItsOwn } from "./retry.ts";

export type SaveStatus = "saved" | "pending" | "saving" | "invalid" | "failed";

const SAVE_AFTER_MS = 1200;
const RETRY_AFTER_MS = 5000;

export class DraftSaver {
  status = $state<SaveStatus>("saved");
  /** When the server last accepted the draft. */
  savedAt = $state<Date | null>(null);
  /** Field problems from the last save, keyed by field path such as `media.0.alt`. */
  errors = $state<Record<string, string>>({});
  /** What went wrong, as a sentence, when status is "invalid" or "failed". */
  message = $state<string | null>(null);
  /** True while a failed save will be tried again by itself. */
  retrying = $state(false);

  #save: (doc: ProjectDoc) => Promise<ProjectEditorResponse>;
  #onsaved: (response: ProjectEditorResponse) => void;
  #latest: ProjectDoc;
  #savedJson: string;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #running: Promise<void> | null = null;

  constructor(options: {
    initial: ProjectDoc;
    savedAt: Date | null;
    save: (doc: ProjectDoc) => Promise<ProjectEditorResponse>;
    onsaved: (response: ProjectEditorResponse) => void;
  }) {
    this.#latest = options.initial;
    this.#savedJson = JSON.stringify(options.initial);
    this.savedAt = options.savedAt;
    this.#save = options.save;
    this.#onsaved = options.onsaved;
  }

  /** True when the latest change has not reached the server. */
  get dirty(): boolean {
    return JSON.stringify(this.#latest) !== this.#savedJson;
  }

  /** True when the server already has exactly this draft. */
  isSaved(doc: ProjectDoc): boolean {
    return JSON.stringify(doc) === this.#savedJson;
  }

  /** Call after every edit with a plain copy of the draft. */
  change(doc: ProjectDoc): void {
    this.#latest = doc;
    clearTimeout(this.#timer);
    if (!this.dirty) {
      if (this.status === "pending") this.status = "saved";
      return;
    }
    this.status = "pending";
    this.#timer = setTimeout(() => void this.#run(), SAVE_AFTER_MS);
  }

  /** Saves now if anything is waiting. True when the server has the latest draft. */
  async flush(): Promise<boolean> {
    clearTimeout(this.#timer);
    if (this.#running !== null) await this.#running;
    // A change made while that save ran still needs its own.
    clearTimeout(this.#timer);
    if (this.dirty) await this.#run();
    return !this.dirty;
  }

  destroy(): void {
    clearTimeout(this.#timer);
  }

  #run(): Promise<void> {
    this.#running ??= this.#saveLatest().finally(() => {
      this.#running = null;
    });
    return this.#running;
  }

  async #saveLatest(): Promise<void> {
    const doc = this.#latest;
    const json = JSON.stringify(doc);
    if (json === this.#savedJson) {
      this.status = "saved";
      return;
    }
    this.status = "saving";
    try {
      const response = await this.#save(doc);
      this.#savedJson = json;
      this.savedAt = new Date();
      this.errors = {};
      this.message = null;
      this.retrying = false;
      this.#onsaved(response);
      if (this.dirty) {
        this.status = "pending";
        this.#timer = setTimeout(() => void this.#run(), SAVE_AFTER_MS);
      } else {
        this.status = "saved";
      }
    } catch (error) {
      clearTimeout(this.#timer);
      this.retrying = false;
      if (isApiError(error) && error.code === "validation_failed") {
        this.status = "invalid";
        this.errors = { ...error.fields };
        this.message = error.message;
        return;
      }
      this.status = "failed";
      this.message = isApiError(error)
        ? error.message
        : "We couldn\u2019t save your changes. Reload the page and try again.";
      if (retriesOnItsOwn(error)) {
        this.retrying = true;
        this.#timer = setTimeout(() => void this.#run(), RETRY_AFTER_MS);
      }
    }
  }
}
