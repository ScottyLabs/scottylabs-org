<script lang="ts">
  // Several photos in order, the first being the cover: upload, reorder by dragging or with the
  // Move earlier and Move later buttons, describe and remove each one.
  //   <ImageListField label="Photos" bind:value={doc.media} max={8} />
  // Use it inside the portal; uploads send the session's CSRF token.
  import { onDestroy, tick } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import X from "@lucide/svelte/icons/x";
  import type { ImageRef } from "../api/types.ts";
  import { getPortalSession } from "../portal/session.svelte.ts";
  import Field from "../portal/ui/Field.svelte";
  import IconButton from "../portal/ui/IconButton.svelte";
  import AltTextInput from "./AltTextInput.svelte";
  import Dropzone from "./Dropzone.svelte";
  import Img from "./Img.svelte";
  import UploadProgress from "./UploadProgress.svelte";
  import { fileDrop } from "./drop.ts";
  import { focusIsLost, holdsFocus, tileAfter } from "./focus.ts";
  import { UploadTask } from "./upload-task.svelte.ts";
  import { checkImageFile } from "./upload.ts";

  let {
    value = $bindable([]),
    onchange,
    max = 8,
    label = "Photos",
    help = "The first photo is the cover. Drag photos to reorder them, or use the arrow buttons.",
    error,
    altErrors = [],
    altRequired = "all",
    coverLabel = "Cover",
    disabled = false,
  }: {
    value?: ImageRef[];
    onchange?: (images: ImageRef[]) => void;
    /** At most this many photos. */
    max?: number;
    label?: string;
    help?: string;
    error?: string | null;
    /** Alt text errors by photo position, usually from the server's field errors. */
    altErrors?: readonly (string | null | undefined)[];
    /** Which photos need alt text before publishing: every one (galleries) or the first (projects). */
    altRequired?: "all" | "first" | "none";
    /** The badge on the first photo. Pass "" to leave it out. */
    coverLabel?: string;
    disabled?: boolean;
  } = $props();

  /** The first control in a tile: a photo's move or remove buttons, Cancel upload, or Choose photos. */
  const FIRST_CONTROL = "button:not(:disabled), textarea:not(:disabled)";

  const session = getPortalSession();
  const uid = $props.id();
  const labelId = `image-list-${uid}-label`;
  const helpId = `image-list-${uid}-help`;
  const errorId = `image-list-${uid}-error`;

  let queue = $state<UploadTask[]>([]);
  let problems = $state<string[]>([]);
  let announcement = $state("");
  let dragFrom = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  let filesOver = $state(false);
  let list: HTMLOListElement | undefined = $state();
  let addZone: Dropzone | undefined = $state();
  let uploading = false;

  const room = $derived(Math.max(0, max - value.length - queue.length));
  const shownErrors = $derived(error ? [error, ...problems] : problems);
  const keys = $derived(keysFor(value));
  const describedBy = $derived(
    [helpId, shownErrors.length > 0 ? errorId : null].filter((id) => id !== null).join(" "),
  );

  onDestroy(() => {
    for (const task of queue) task.dispose();
  });

  /** Stable keys, even if the same photo appears twice. */
  function keysFor(images: readonly ImageRef[]): string[] {
    const seen = new Map<string, number>();
    return images.map((image) => {
      const count = (seen.get(image.src) ?? 0) + 1;
      seen.set(image.src, count);
      return `${image.src}#${count.toString()}`;
    });
  }

  function emit(next: ImageRef[]): void {
    value = next;
    onchange?.(next);
  }

  function needsAlt(index: number): boolean {
    if (altRequired === "none") return false;
    if (altRequired === "first" && index > 0) return false;
    return value[index]?.alt.trim() === "";
  }

  function add(files: File[]): void {
    if (disabled) return;
    const found: string[] = [];
    const accepted: File[] = [];
    for (const file of files) {
      const invalid = checkImageFile(file);
      if (invalid === null) accepted.push(file);
      else found.push(invalid);
    }
    const taken = accepted.slice(0, room);
    const left = accepted.length - taken.length;
    if (left > 0) {
      found.push(
        `You can add ${room.toString()} more ${room === 1 ? "photo" : "photos"} here, so ${left.toString()} ${left === 1 ? "was" : "were"} left out.`,
      );
    }
    problems = found;
    if (taken.length === 0) return;
    const added = taken.map((file) => new UploadTask(file));
    // The add tile goes away when the list is full; focus then moves to the first upload's Cancel.
    const hadFocus = holdsFocus(list);
    queue = [...queue, ...added];
    announcement = `Uploading ${taken.length.toString()} ${taken.length === 1 ? "photo" : "photos"}.`;
    if (hadFocus) void refocus(() => focusIn(pendingTile(added[0]), "button"));
    void uploadQueued();
  }

  /** Uploads one photo at a time, so they arrive in the order chosen. */
  async function uploadQueued(): Promise<void> {
    if (uploading) return;
    uploading = true;
    try {
      let task = queue[0];
      while (task !== undefined) {
        // One at a time on purpose: photos arrive in the order they were chosen.
        // oxlint-disable-next-line no-await-in-loop
        const image = await task.run(session.csrfToken);
        // The finished upload's tile is replaced by its photo, or removed when it failed or was
        // cancelled, so focus on its Cancel upload button moves to what took its place.
        const hadFocus = holdsFocus(list);
        const position = value.length;
        if (image === null && task.status === "failed") {
          problems = [...problems, task.error ?? `${task.file.name} did not upload.`];
          if (task.failedStatus === 401) session.markSignedOut("expired");
        } else if (image !== null) {
          emit([...value, { ...image, alt: "" }]);
        }
        const finished = task;
        queue = queue.filter((queued) => queued !== finished);
        finished.dispose();
        if (hadFocus) {
          // A new photo needs alt text next; otherwise go to the tile that moved up.
          void refocus(() => focusIn(tileAt(position), image === null ? FIRST_CONTROL : "textarea"));
        }
        task = queue[0];
      }
      announcement = "Uploads finished.";
    } finally {
      uploading = false;
    }
  }

  function cancel(task: UploadTask): void {
    task.cancel();
    // The running upload leaves the queue when run() settles, in uploadQueued().
    if (queue[0] !== task) {
      const hadFocus = holdsFocus(list);
      const position = value.length + queue.indexOf(task);
      queue = queue.filter((queued) => queued !== task);
      task.dispose();
      if (hadFocus) void refocus(() => focusIn(tileAt(position), FIRST_CONTROL));
    }
    announcement = `Upload of ${task.file.name} cancelled.`;
  }

  /** The tile at `position`, or the last one when the list got shorter. */
  function tileAt(position: number): Element | undefined {
    const tiles = list?.children;
    if (tiles === undefined) return undefined;
    const index = tileAfter(position, tiles.length);
    return index === null ? undefined : tiles[index];
  }

  function pendingTile(task: UploadTask | undefined): Element | undefined {
    if (task === undefined) return undefined;
    return list?.querySelector(`[data-task="${CSS.escape(task.key)}"]`) ?? undefined;
  }

  function focusIn(tile: Element | undefined, selector: string): void {
    tile?.querySelector<HTMLElement>(selector)?.focus();
  }

  /** Once the page has updated, calls `moveFocus` if the element that had focus went away. */
  async function refocus(moveFocus: () => void): Promise<void> {
    await tick();
    if (focusIsLost()) moveFocus();
  }

  function moved(from: number, to: number): ImageRef[] {
    const next = [...value];
    const [image] = next.splice(from, 1);
    if (image !== undefined) next.splice(to, 0, image);
    return next;
  }

  async function move(from: number, to: number, focusAction?: "earlier" | "later"): Promise<void> {
    if (to < 0 || to >= value.length || from === to) return;
    const key = keys[from];
    emit(moved(from, to));
    const cover = to === 0 && coverLabel !== "" ? ` It is now the ${coverLabel.toLowerCase()}.` : "";
    announcement = `Photo moved to position ${(to + 1).toString()} of ${value.length.toString()}.${cover}`;
    if (focusAction === undefined) return;
    // Moving the tile takes focus with it; put it back on the same button, or its partner at an end.
    await tick();
    const tile = list?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key ?? "")}"]`);
    const wanted = tile?.querySelector<HTMLButtonElement>(`[data-action="${focusAction}"]:not(:disabled)`);
    const other = tile?.querySelector<HTMLButtonElement>(`[data-action]:not(:disabled)`);
    (wanted ?? other)?.focus();
  }

  async function remove(index: number): Promise<void> {
    emit(value.filter((_, i) => i !== index));
    announcement = `Photo ${(index + 1).toString()} removed.`;
    await tick();
    const tiles = list?.querySelectorAll<HTMLElement>("[data-key]");
    const next = tiles?.[tileAfter(index, tiles.length) ?? -1];
    const button = next?.querySelector<HTMLButtonElement>('[data-action="remove"]');
    if (button) button.focus();
    else addZone?.focus();
  }

  function setAlt(index: number, alt: string): void {
    emit(value.map((image, i) => (i === index ? { ...image, alt } : image)));
  }

  function onTileDrop(event: DragEvent): void {
    if (dragFrom === null || dropAt === null) return;
    event.preventDefault();
    const from = dragFrom;
    const to = dropAt > from ? dropAt - 1 : dropAt;
    dragFrom = null;
    dropAt = null;
    void move(from, to);
  }

  function onTileDragEnd(): void {
    dragFrom = null;
    dropAt = null;
  }

  // Dragging a tile's picture starts a reorder. The listener sits on the picture, not the tile, so
  // dragging selected text out of an alt text field stays a text drag.
  function dragHandle(index: number): Attachment<HTMLElement> {
    const onDragStart = (event: DragEvent) => {
      if (disabled || event.dataTransfer === null) return;
      dragFrom = index;
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", value[index]?.src ?? "");
    };
    return (node) => {
      node.addEventListener("dragstart", onDragStart);
      return () => node.removeEventListener("dragstart", onDragStart);
    };
  }

  // Where a dragged picture lands. Files dragged from the desktop are left to the list's drop zone.
  function dropSpot(index: number): Attachment<HTMLElement> {
    return (node) => {
      const onDragOver = (event: DragEvent) => {
        if (dragFrom === null) return;
        event.preventDefault();
        if (event.dataTransfer !== null) event.dataTransfer.dropEffect = "move";
        const box = node.getBoundingClientRect();
        const after = event.clientX > box.left + box.width / 2;
        dropAt = index + (after ? 1 : 0);
      };
      node.addEventListener("dragover", onDragOver);
      node.addEventListener("drop", onTileDrop);
      node.addEventListener("dragend", onTileDragEnd);
      return () => {
        node.removeEventListener("dragover", onDragOver);
        node.removeEventListener("drop", onTileDrop);
        node.removeEventListener("dragend", onTileDragEnd);
      };
    };
  }
</script>

<div class="image-list" role="group" aria-labelledby={labelId} aria-describedby={describedBy}>
  <div class="image-list__top">
    <span id={labelId} class="image-list__label">{label}</span>
    <span class="image-list__count">
      {value.length} of {max}
    </span>
  </div>
  <p id={helpId} class="image-list__help">
    {help}{#if altRequired !== "none"}
      {" "}{altRequired === "all"
        ? "Every photo needs alt text before you publish."
        : "The cover needs alt text before you publish."}{/if}
  </p>

  <ol
    bind:this={list}
    class={["image-list__grid", filesOver && "image-list__grid--files"]}
    {@attach fileDrop({ onfiles: add, onactive: (on) => (filesOver = on), disabled: disabled || room === 0 })}
  >
    {#each value as image, index (keys[index])}
      <li
        class={[
          "tile",
          dragFrom === index && "tile--dragging",
          dropAt === index && dragFrom !== null && "tile--drop-before",
          dropAt === index + 1 && dragFrom !== null && index === value.length - 1 && "tile--drop-after",
        ]}
        data-key={keys[index]}
        {@attach dropSpot(index)}
      >
        <!-- Only the picture is draggable, so text in the alt field can still be selected. -->
        <div class="tile__thumb" draggable={disabled ? "false" : "true"} {@attach dragHandle(index)}>
          <Img {image} alt="" sizes="200px" fallbackWidth={480} fit="cover" />
          <span class="tile__grip" aria-hidden="true"><GripVertical size={14} strokeWidth={2} /></span>
          <span class="tile__badges">
            {#if index === 0 && coverLabel !== ""}
              <span class="tile__badge">{coverLabel}</span>
            {/if}
            {#if needsAlt(index)}
              <span class="tile__badge tile__badge--warning">Needs alt text</span>
            {/if}
          </span>
        </div>
        <div class="tile__actions">
          <span class="tile__position">Photo {index + 1}</span>
          <IconButton
            data-action="earlier"
            icon={ArrowLeft}
            label={`Move photo ${(index + 1).toString()} earlier`}
            size="sm"
            disabled={disabled || index === 0}
            onclick={() => move(index, index - 1, "earlier")}
          />
          <IconButton
            data-action="later"
            icon={ArrowRight}
            label={`Move photo ${(index + 1).toString()} later`}
            size="sm"
            disabled={disabled || index === value.length - 1}
            onclick={() => move(index, index + 1, "later")}
          />
          <IconButton
            data-action="remove"
            icon={X}
            label={`Remove photo ${(index + 1).toString()}`}
            size="sm"
            {disabled}
            onclick={() => remove(index)}
          />
        </div>
        <Field label={`Alt text for photo ${(index + 1).toString()}`} error={altErrors[index]}>
          <AltTextInput
            value={image.alt}
            {disabled}
            placeholder="Describe the photo"
            onchange={(alt) => setAlt(index, alt)}
          />
        </Field>
      </li>
    {/each}
    {#each queue as task (task.key)}
      <li class="tile tile--pending" data-task={task.key}>
        <UploadProgress {task} compact oncancel={() => cancel(task)} />
      </li>
    {/each}
    {#if room > 0}
      <li class="tile tile--add">
        <Dropzone
          bind:this={addZone}
          layout="tile"
          multiple
          title={value.length === 0 ? "Drop photos here" : "Add more photos"}
          buttonLabel="Choose photos"
          {disabled}
          {describedBy}
          onfiles={add}
        />
      </li>
    {/if}
  </ol>

  <div id={errorId} class="image-list__errors" aria-live="polite">
    {#each shownErrors as message, index (index)}
      <p>{message}</p>
    {/each}
  </div>
  <span class="p-sr-only" aria-live="polite">{announcement}</span>
</div>

<style>
  .image-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .image-list__top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .image-list__label {
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .image-list__count {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
  }
  .image-list__help {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .image-list__grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
    gap: 16px;
    margin: 4px 0 0;
    padding: 0;
    border-radius: var(--p-radius);
    list-style: none;
    transition: box-shadow var(--p-duration) var(--p-ease);
  }
  .image-list__grid--files {
    box-shadow: 0 0 0 2px var(--p-focus);
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .tile--dragging {
    opacity: 0.45;
  }
  .tile--drop-before::before,
  .tile--drop-after::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 3px;
    border-radius: 2px;
    background: var(--p-focus);
  }
  .tile--drop-before::before {
    left: -10px;
  }
  .tile--drop-after::after {
    right: -10px;
  }
  .tile__thumb {
    position: relative;
    overflow: hidden;
    aspect-ratio: 4 / 3;
    border-radius: var(--p-radius);
    background: var(--p-surface-strong);
    box-shadow: inset 0 0 0 1px var(--p-border-muted);
    cursor: grab;
  }
  .tile__thumb:active {
    cursor: grabbing;
  }
  .tile__grip {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.9);
    color: var(--p-text);
  }
  .tile__badges {
    position: absolute;
    bottom: 6px;
    left: 6px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .tile__badge {
    padding: 2px 8px;
    border-radius: var(--p-radius-pill);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-size: 0.6875rem;
    font-weight: 500;
    line-height: 16px;
  }
  .tile__badge--warning {
    background: var(--p-warning-surface);
    color: var(--p-warning-text);
  }
  .tile__actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .tile__position {
    flex: 1;
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .tile--pending,
  .tile--add {
    min-height: 200px;
  }
  .tile--add:only-child {
    grid-column: 1 / -1;
    min-height: 0;
  }
  .image-list__errors {
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;

    &:empty {
      margin-top: -8px;
    }
    & p {
      margin: 0;
      font-size: inherit;
    }
  }
</style>
