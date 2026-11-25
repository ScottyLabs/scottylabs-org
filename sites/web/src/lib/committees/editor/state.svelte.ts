// The page editor's state: the draft, the selection, undo and redo, autosave and the edit lock.
// Components change the page only through these methods, so every change lands in the history and gets saved.

import { isApiError } from "../../api/client.ts";
import type {
  Block,
  BlockWidth,
  CommitteePageDoc,
  CommitteePageEditorResponse,
  CommitteePageStatus,
  CommitteeSettings,
  CommitteeSlug,
  EditLock,
  ProjectCard,
} from "../../api/types.ts";
import { type AddableType, MAX_BLOCKS, duplicateBlock, newBlock } from "../blocks.ts";
import { NO_PROBLEMS, type PageProblems, problemsFor } from "../errors.ts";
import { History } from "../history.ts";
import { moveBlock } from "../layout.ts";
import type { CommitteeInfo } from "../render/types.ts";
import { loadEditor, releaseLock, saveDraft, takeLock } from "../api.ts";

/** Saved within this long after the last change, and never later than MAX_WAIT_MS after the first. */
const SAVE_DELAY_MS = 700;
const MAX_WAIT_MS = 1800;
const RETRY_MS = 5000;
/** The lock lasts five minutes; refreshing every minute keeps it while the editor is open. */
export const HEARTBEAT_MS = 60_000;

export type SaveState = "saved" | "unsaved" | "saving" | "offline" | "invalid" | "conflict";

export class PageEditor {
  readonly slug: CommitteeSlug;
  readonly committee: CommitteeInfo;
  readonly canEdit: boolean;

  doc = $state.raw<CommitteePageDoc>({ blocks: [], settings: emptySettings() });
  published = $state.raw<CommitteePageDoc | null>(null);
  projects = $state.raw<readonly ProjectCard[]>([]);
  status = $state<CommitteePageStatus>("published");
  /** The draft version the next save builds on. */
  version = $state(0);
  savedAt = $state<string | null>(null);
  saveState = $state<SaveState>("saved");
  /** Says what went wrong with the last save, as the service put it. */
  saveMessage = $state<string | null>(null);
  problems = $state.raw<PageProblems>(NO_PROBLEMS);
  selectedId = $state<string | null>(null);
  lock = $state.raw<EditLock | null>(null);
  /** Why the page is read only right now, or null while this person may edit. */
  readOnlyReason = $state<string | null>(null);
  #historyVersion = $state(0);

  #history = new History<CommitteePageDoc>();
  /** Counts changes; a save records which one it sent. */
  #revision = 0;
  #savedRevision = 0;
  #saving: Promise<void> | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #firstPendingAt: number | null = null;
  #heartbeat: ReturnType<typeof setInterval> | null = null;
  #destroyed = false;

  constructor(response: CommitteePageEditorResponse) {
    this.slug = response.committee.slug;
    this.committee = { ...response.committee };
    this.canEdit = response.canEdit;
    this.#apply(response);
    if (!this.canEdit) {
      this.readOnlyReason = `Only the director of ${response.committee.name} or an admin can edit this page.`;
    }
  }

  // -------------------------------------------------------------------------
  // Reading

  get writable(): boolean {
    return this.canEdit && this.readOnlyReason === null;
  }

  get canUndo(): boolean {
    void this.#historyVersion;
    return this.writable && this.#history.canUndo;
  }

  get canRedo(): boolean {
    void this.#historyVersion;
    return this.writable && this.#history.canRedo;
  }

  get selected(): Block | null {
    return this.doc.blocks.find((block) => block.id === this.selectedId) ?? null;
  }

  get canAddBlock(): boolean {
    return this.writable && this.doc.blocks.length < MAX_BLOCKS;
  }

  /** True while changes wait to be saved or are being saved. */
  get hasPendingChanges(): boolean {
    return (
      this.saveState === "unsaved" || this.saveState === "saving" || this.saveState === "offline"
    );
  }

  // -------------------------------------------------------------------------
  // Changing the page

  select(id: string | null): void {
    this.selectedId = id;
    this.#history.seal();
  }

  /** Replaces the page. `key` merges quick changes to one field into one undo step. */
  change(next: CommitteePageDoc, key: string | null = null): void {
    if (!this.writable) return;
    this.#history.record(this.doc, key);
    this.doc = next;
    this.#historyVersion += 1;
    this.#markChanged();
  }

  updateBlock(block: Block, key: string | null = null): void {
    this.change(
      { ...this.doc, blocks: this.doc.blocks.map((b) => (b.id === block.id ? block : b)) },
      key,
    );
  }

  updateSettings(settings: Partial<CommitteeSettings>, key: string | null = null): void {
    this.change({ ...this.doc, settings: { ...this.doc.settings, ...settings } }, key);
  }

  /** Adds a block at `index` (the end by default) and selects it. Returns its id. */
  addBlock(type: AddableType, index?: number, width?: BlockWidth): string | null {
    if (!this.canAddBlock) return null;
    const block = newBlock(type, this.committee.name, width);
    const blocks = [...this.doc.blocks];
    blocks.splice(Math.max(1, Math.min(index ?? blocks.length, blocks.length)), 0, block);
    this.change({ ...this.doc, blocks });
    this.selectedId = block.id;
    return block.id;
  }

  /** Moves a block so it ends up at position `to`. The cover stays first. Steps of one drag share `key`. */
  moveBlock(id: string, to: number, key: string | null = null): boolean {
    const from = this.doc.blocks.findIndex((block) => block.id === id);
    if (from <= 0) return false;
    const blocks = moveBlock(this.doc.blocks, from, to);
    if (blocks.every((block, index) => block === this.doc.blocks[index])) return false;
    this.change({ ...this.doc, blocks }, key);
    return true;
  }

  moveBy(id: string, step: -1 | 1): boolean {
    const from = this.doc.blocks.findIndex((block) => block.id === id);
    return this.moveBlock(id, from + step);
  }

  duplicate(id: string): string | null {
    const index = this.doc.blocks.findIndex((block) => block.id === id);
    const block = this.doc.blocks[index];
    if (block === undefined || block.type === "cover" || !this.canAddBlock) return null;
    const copy = duplicateBlock(block);
    const blocks = [...this.doc.blocks];
    blocks.splice(index + 1, 0, copy);
    this.change({ ...this.doc, blocks });
    this.selectedId = copy.id;
    return copy.id;
  }

  remove(id: string): boolean {
    const block = this.doc.blocks.find((b) => b.id === id);
    if (block === undefined || block.type === "cover") return false;
    this.change({ ...this.doc, blocks: this.doc.blocks.filter((b) => b.id !== id) });
    if (this.selectedId === id) this.selectedId = null;
    return true;
  }

  setWidth(id: string, width: BlockWidth): void {
    const block = this.doc.blocks.find((b) => b.id === id);
    if (block === undefined || block.width === width) return;
    this.updateBlock({ ...block, width });
  }

  undo(): void {
    if (!this.writable) return;
    const previous = this.#history.undo(this.doc);
    if (previous === null) return;
    this.doc = previous;
    this.#afterTimeTravel();
  }

  redo(): void {
    if (!this.writable) return;
    const next = this.#history.redo(this.doc);
    if (next === null) return;
    this.doc = next;
    this.#afterTimeTravel();
  }

  #afterTimeTravel(): void {
    this.#historyVersion += 1;
    if (this.selectedId !== null && !this.doc.blocks.some((b) => b.id === this.selectedId)) {
      this.selectedId = null;
    }
    this.#markChanged();
  }

  // -------------------------------------------------------------------------
  // Saving

  #markChanged(): void {
    this.#revision += 1;
    if (this.saveState !== "conflict")
      this.saveState = this.#saving === null ? "unsaved" : "saving";
    this.#firstPendingAt ??= Date.now();
    this.#schedule(SAVE_DELAY_MS);
  }

  #schedule(delay: number): void {
    if (this.#timer !== null) clearTimeout(this.#timer);
    const waited = this.#firstPendingAt === null ? 0 : Date.now() - this.#firstPendingAt;
    const wait = Math.max(0, Math.min(delay, MAX_WAIT_MS - waited));
    this.#timer = setTimeout(() => {
      this.#timer = null;
      void this.#saveNow();
    }, wait);
  }

  /** Saves now if anything is waiting, and resolves once the page is saved or cannot be. */
  async flush(): Promise<boolean> {
    if (this.#timer !== null) {
      clearTimeout(this.#timer);
      this.#timer = null;
    }
    // A save that finishes schedules the next one on a timer, so one wait covers what is in flight.
    if (this.#saving !== null) await this.#saving;
    if (this.#timer !== null) {
      clearTimeout(this.#timer);
      this.#timer = null;
    }
    if (this.#revision !== this.#savedRevision && this.writable && this.saveState !== "conflict") {
      await this.#saveNow();
    }
    return this.#revision === this.#savedRevision;
  }

  #saveNow(): Promise<void> {
    if (this.#saving !== null || this.#destroyed) return this.#saving ?? Promise.resolve();
    if (this.#revision === this.#savedRevision || !this.writable || this.saveState === "conflict") {
      return Promise.resolve();
    }
    this.#saving = this.#send().finally(() => {
      this.#saving = null;
      if (this.#revision !== this.#savedRevision && this.saveState === "unsaved") this.#schedule(0);
    });
    return this.#saving;
  }

  async #send(): Promise<void> {
    const revision = this.#revision;
    const doc = this.doc;
    this.saveState = "saving";
    this.#firstPendingAt = null;
    try {
      const saved = await saveDraft(this.slug, doc, this.version);
      this.version = saved.draftVersion;
      this.status = saved.status;
      this.savedAt = saved.savedAt;
      this.#savedRevision = revision;
      this.problems = NO_PROBLEMS;
      this.saveMessage = null;
      this.saveState = this.#revision === revision ? "saved" : "unsaved";
    } catch (error) {
      await this.#saveFailed(error, doc);
    }
  }

  async #saveFailed(error: unknown, doc: CommitteePageDoc): Promise<void> {
    if (!isApiError(error)) throw error;
    this.saveMessage = error.message;
    if (error.status === 422) {
      this.problems = problemsFor(doc, error.fields);
      this.saveState = "invalid";
    } else if (error.status === 409) {
      await this.#conflict(error.message);
    } else if (error.status === 401 || error.status === 403) {
      this.readOnlyReason = error.message;
      this.saveState = "conflict";
    } else {
      // Offline or a server hiccup: keep the changes and try again shortly.
      this.saveState = "offline";
      this.#firstPendingAt = Date.now();
      this.#schedule(RETRY_MS);
    }
  }

  /** A 409 means someone else took the lock or saved a newer version. Reading the page says which. */
  async #conflict(message: string): Promise<void> {
    this.saveState = "conflict";
    try {
      const latest = await loadEditor(this.slug);
      if (latest.lock !== null && !latest.lock.mine) {
        this.lock = latest.lock;
        this.readOnlyReason = message;
      }
    } catch {
      // The message from the save already says what to do.
    }
  }

  // -------------------------------------------------------------------------
  // The edit lock

  /** Takes the lock and keeps it with a heartbeat. Leaves the page read only when someone else holds it. */
  async start(): Promise<void> {
    if (!this.canEdit) return;
    await this.#claim();
    this.#heartbeat ??= setInterval(() => void this.#claim(), HEARTBEAT_MS);
  }

  async #claim(): Promise<void> {
    if (this.#destroyed) return;
    try {
      this.lock = await takeLock(this.slug);
      if (this.readOnlyReason !== null && this.saveState !== "conflict") this.readOnlyReason = null;
    } catch (error) {
      if (!isApiError(error)) throw error;
      if (error.status === 409) {
        this.readOnlyReason = error.message;
        await this.refresh();
      }
    }
  }

  /** Tries again to edit a page someone else was editing, with their latest draft. */
  async retry(): Promise<void> {
    await this.refresh();
    if (this.lock === null || this.lock.mine) {
      this.readOnlyReason = null;
      this.saveState = "saved";
      await this.start();
    }
  }

  /** Loads the latest draft from the service, dropping the history. Only while nothing waits to be saved. */
  async refresh(): Promise<void> {
    const latest = await loadEditor(this.slug);
    if (this.hasPendingChanges && this.writable) return;
    this.#apply(latest);
  }

  /** Takes the page as the service has it, after a reload, publish or discard. */
  replace(response: CommitteePageEditorResponse): void {
    this.#apply(response);
    this.saveState = "saved";
    this.saveMessage = null;
    this.problems = NO_PROBLEMS;
  }

  #apply(response: CommitteePageEditorResponse): void {
    this.doc = response.draft;
    this.published = response.published;
    this.projects = response.projects;
    this.status = response.status;
    this.version = response.draftVersion;
    this.lock = response.lock;
    this.#revision = 0;
    this.#savedRevision = 0;
    this.#history.clear();
    this.#historyVersion += 1;
    if (this.selectedId !== null && !this.doc.blocks.some((b) => b.id === this.selectedId)) {
      this.selectedId = null;
    }
  }

  /** Stops the timers. Call `releaseLock` separately so it can wait for a last save. */
  destroy(): void {
    this.#destroyed = true;
    if (this.#timer !== null) clearTimeout(this.#timer);
    if (this.#heartbeat !== null) clearInterval(this.#heartbeat);
  }

  /** Saves what is waiting and lets go of the lock, for leaving the editor inside the portal. */
  async leave(): Promise<void> {
    await this.flush();
    this.destroy();
    if (this.lock?.mine === true) {
      try {
        await releaseLock(this.slug);
      } catch {
        // The lock expires by itself after five minutes.
      }
    }
  }
}

function emptySettings(): CommitteeSettings {
  return {
    indexDescription: "",
    teamsLine: "",
    meetingTime: "",
    meetingRoom: "",
    mapUrl: null,
    slackChannel: null,
  };
}
