<script lang="ts">
  // One photo: drop or choose it, watch it upload, then replace or remove it and describe it.
  //   <ImageField label="Photo" bind:value={block.props.image} altError={errors["image.alt"]} />
  // Use it inside the portal; uploads send the session's CSRF token.
  import { onDestroy, tick } from "svelte";
  import type { ImageRef } from "../api/types.ts";
  import { getPortalSession } from "../portal/session.svelte.ts";
  import Button from "../portal/ui/Button.svelte";
  import Field from "../portal/ui/Field.svelte";
  import Dropzone from "./Dropzone.svelte";
  import Img from "./Img.svelte";
  import AltTextInput from "./AltTextInput.svelte";
  import UploadProgress from "./UploadProgress.svelte";
  import { type UploadEnd, focusAfterUpload, focusIsLost, holdsFocus } from "./focus.ts";
  import { aspectRatio } from "./images.ts";
  import { UploadTask } from "./upload-task.svelte.ts";
  import { checkImageFile } from "./upload.ts";

  let {
    value = $bindable(null),
    onchange,
    label = "Photo",
    help,
    error,
    altError,
    altRequired = true,
    disabled = false,
    ratio,
  }: {
    value?: ImageRef | null;
    onchange?: (image: ImageRef | null) => void;
    label?: string;
    /** A sentence under the label, such as the size that works best. */
    help?: string;
    /** About the photo itself, such as "Add a cover photo before you publish." */
    error?: string | null;
    /** About the alt text, usually from the server's field errors. */
    altError?: string | null;
    /** Says alt text is needed before publishing, as it is for every page and cover photo. */
    altRequired?: boolean;
    disabled?: boolean;
    /** Width over height of the preview. Defaults to the photo's own shape. */
    ratio?: number;
  } = $props();

  const session = getPortalSession();
  const uid = $props.id();
  const labelId = `image-field-${uid}-label`;
  const helpId = `image-field-${uid}-help`;
  const errorId = `image-field-${uid}-error`;

  let task = $state<UploadTask | null>(null);
  let problem = $state<string | null>(null);
  let announcement = $state("");
  let root: HTMLDivElement | undefined = $state();
  let dropzone: Dropzone | undefined = $state();
  let progress: UploadProgress | undefined = $state();

  const shownError = $derived(problem ?? error ?? null);
  const describedBy = $derived(
    [help ? helpId : null, shownError ? errorId : null].filter((id) => id !== null).join(" ") ||
      undefined,
  );

  onDestroy(() => task?.dispose());

  function emit(next: ImageRef | null): void {
    value = next;
    onchange?.(next);
  }

  async function start(files: File[]): Promise<void> {
    const file = files[0];
    if (file === undefined || disabled) return;
    const invalid = checkImageFile(file);
    if (invalid !== null) {
      problem = invalid;
      return;
    }
    problem = null;
    task?.dispose();
    const current = new UploadTask(file);
    // Replace photo and Choose a photo go away while the upload runs; Cancel upload takes over.
    const hadFocus = holdsFocus(root);
    task = current;
    announcement =
      files.length > 1
        ? `Uploading ${file.name}. Only the first photo is used here.`
        : `Uploading ${file.name}.`;
    if (hadFocus) void refocus(() => progress?.focus());

    const image = await current.run(session.csrfToken);
    if (task !== current) return;
    let end: UploadEnd = "cancelled";
    if (image !== null) {
      end = "done";
      // A new photo needs its own description.
      emit({ ...image, alt: "" });
      announcement = "Photo uploaded. Describe it in the alt text field.";
    } else if (current.status === "failed") {
      end = "failed";
      problem = current.error;
      if (current.failedStatus === 401) session.markSignedOut("expired");
    } else {
      announcement = "Upload cancelled.";
    }
    const focusWasHere = holdsFocus(root);
    task = null;
    current.dispose();
    if (focusWasHere) void refocus(() => focusTarget(focusAfterUpload(end, value !== null)));
  }

  /** Once the page has updated, calls `moveFocus` if the element that had focus went away. */
  async function refocus(moveFocus: () => void): Promise<void> {
    await tick();
    if (focusIsLost()) moveFocus();
  }

  function focusTarget(target: "alt" | "replace" | "choose"): void {
    if (target === "choose") {
      dropzone?.focus();
      return;
    }
    const selector = target === "alt" ? "textarea" : '[data-action="replace"]';
    root?.querySelector<HTMLElement>(selector)?.focus();
  }

  function cancel(): void {
    task?.cancel();
  }

  async function remove(): Promise<void> {
    emit(null);
    announcement = "Photo removed.";
    // The Remove button is gone, so focus moves to the button that adds a photo again.
    await tick();
    dropzone?.focus();
  }

  function setAlt(alt: string): void {
    if (value === null) return;
    emit({ ...value, alt });
  }
</script>

<div bind:this={root} class="image-field" role="group" aria-labelledby={labelId}>
  <div class="image-field__top">
    <span id={labelId} class="image-field__label">{label}</span>
  </div>
  {#if help}<p id={helpId} class="image-field__help">{help}</p>{/if}

  {#if task !== null}
    <UploadProgress bind:this={progress} {task} oncancel={cancel} />
  {:else if value !== null}
    <div class="image-field__preview" style:aspect-ratio={ratio ?? aspectRatio(value)}>
      <Img image={value} alt="" sizes="320px" fallbackWidth={480} fit="cover" loading="eager" />
    </div>
    <div class="image-field__actions">
      <Button
        variant="outline"
        size="sm"
        {disabled}
        aria-describedby={describedBy}
        data-action="replace"
        onclick={() => dropzone?.choose()}
      >
        Replace photo
      </Button>
      <Button variant="subtle" size="sm" {disabled} onclick={remove}>Remove photo</Button>
    </div>
  {/if}
  <!-- Kept mounted while a photo is shown, so Replace can open its file picker. -->
  <div class={["image-field__drop", (task !== null || value !== null) && "image-field__drop--hidden"]}>
    <Dropzone bind:this={dropzone} {disabled} describedBy={describedBy} onfiles={start} />
  </div>

  <div id={errorId} class="image-field__error" aria-live="polite">{shownError ?? ""}</div>
  <span class="p-sr-only" aria-live="polite">{announcement}</span>

  {#if value !== null}
    <Field
      label="Alt text"
      help={altRequired ? "Required before you publish." : "Helps people who use screen readers."}
      error={altError}
    >
      <AltTextInput value={value.alt} {disabled} onchange={setAlt} />
    </Field>
  {/if}
</div>

<style>
  .image-field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .image-field__label {
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .image-field__help {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .image-field__preview {
    overflow: hidden;
    width: 100%;
    max-height: 320px;
    border-radius: var(--p-radius);
    background: var(--p-surface-strong);
    box-shadow: inset 0 0 0 1px var(--p-border-muted);
  }
  .image-field__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .image-field__drop--hidden {
    display: none;
  }
  .image-field__error {
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;

    &:empty {
      margin-top: -8px;
    }
  }
</style>
