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
  let dropzone: Dropzone | undefined = $state();

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
    if (files.length > 1) announcement = "Only the first photo is used here.";
    problem = null;
    task?.dispose();
    const current = new UploadTask(file);
    task = current;
    announcement = `Uploading ${file.name}.`;
    const image = await current.run(session.csrfToken);
    if (task !== current) return;
    if (image !== null) {
      // A new photo needs its own description.
      emit({ ...image, alt: "" });
      announcement = "Photo uploaded. Describe it in the alt text field.";
    } else if (current.status === "failed") {
      problem = current.error;
      if (current.failedStatus === 401) session.markSignedOut("expired");
    } else {
      announcement = "Upload cancelled.";
    }
    task = null;
    current.dispose();
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

<div class="image-field" role="group" aria-labelledby={labelId}>
  <div class="image-field__top">
    <span id={labelId} class="image-field__label">{label}</span>
  </div>
  {#if help}<p id={helpId} class="image-field__help">{help}</p>{/if}

  {#if task !== null}
    <UploadProgress {task} oncancel={cancel} />
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
