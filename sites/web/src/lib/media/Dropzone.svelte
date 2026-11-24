<script lang="ts">
  // A dashed area that takes dropped photos, with a button that opens the file picker. The
  // button is the accessible way in; the area also opens the picker when clicked.
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import Button from "../portal/ui/Button.svelte";
  import { fileDrop } from "./drop.ts";
  import { ACCEPT, UPLOAD_HINT } from "./upload.ts";

  let {
    title = "Drop a photo here",
    hint = UPLOAD_HINT,
    buttonLabel = "Choose a photo",
    multiple = false,
    disabled = false,
    layout = "panel",
    describedBy,
    onfiles,
  }: {
    title?: string;
    hint?: string;
    buttonLabel?: string;
    multiple?: boolean;
    disabled?: boolean;
    /** "panel" puts the button under the area; "tile" puts it inside, for image grids. */
    layout?: "panel" | "tile";
    /** Ids of help or error text for the button. */
    describedBy?: string;
    onfiles: (files: File[]) => void;
  } = $props();

  let root: HTMLDivElement | undefined = $state();
  let input: HTMLInputElement | undefined = $state();
  let active = $state(false);

  /** Opens the file picker. */
  export function choose(): void {
    if (!disabled) input?.click();
  }

  /** Focuses the button that opens the file picker. */
  export function focus(): void {
    root?.querySelector("button")?.focus();
  }

  function onchange(): void {
    const files = Array.from(input?.files ?? []);
    // Clearing lets the same file be chosen again after a failed upload.
    if (input !== undefined) input.value = "";
    if (files.length > 0) onfiles(files);
  }

  // A click anywhere in the area opens the picker too, except on its own button.
  function onAreaClick(event: MouseEvent): void {
    if (event.target instanceof Element && event.target.closest("button") !== null) return;
    choose();
  }

  function openPicker(node: HTMLElement) {
    node.addEventListener("click", onAreaClick);
    return () => node.removeEventListener("click", onAreaClick);
  }
</script>

<div bind:this={root} class={["dropzone", `dropzone--${layout}`]}>
  <div
    class={["dropzone__area", active && "dropzone__area--active", disabled && "dropzone__area--disabled"]}
    {@attach fileDrop({ onfiles, onactive: (on) => (active = on), disabled })}
    {@attach openPicker}
  >
    <ImagePlus size={20} strokeWidth={1.75} aria-hidden="true" />
    <span class="dropzone__title">{active ? "Drop to upload" : title}</span>
    <span class="dropzone__hint">{hint}</span>
    {#if layout === "tile"}
      <Button
        variant="outline"
        size="sm"
        {disabled}
        aria-describedby={describedBy}
        onclick={choose}
      >
        {buttonLabel}
      </Button>
    {/if}
  </div>
  {#if layout === "panel"}
    <Button variant="outline" size="sm" {disabled} aria-describedby={describedBy} onclick={choose}>
      {buttonLabel}
    </Button>
  {/if}
  <input bind:this={input} type="file" accept={ACCEPT} {multiple} hidden {onchange} />
</div>

<style>
  .dropzone {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
  }
  .dropzone--tile {
    height: 100%;
  }
  .dropzone__area {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    min-height: 114px;
    padding: 22px 12px;
    border: 1px dashed var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
    color: var(--p-text);
    font-family: var(--p-font);
    text-align: center;
    cursor: pointer;
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease);

    &:hover {
      border-color: #b8b8b8;
    }
  }
  .dropzone--tile .dropzone__area {
    flex: 1;
    padding: 16px 10px;
  }
  .dropzone__area--active {
    border-style: solid;
    border-color: var(--p-focus);
    background: var(--p-info-surface);
  }
  .dropzone__area--disabled {
    color: var(--p-text-disabled);
    cursor: not-allowed;
  }
  .dropzone__title {
    font-size: 0.875rem;
    font-weight: 400;
    line-height: 20px;
  }
  .dropzone__hint {
    color: var(--p-text-tertiary);
    text-wrap: balance;
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
  }
  .dropzone__area--disabled .dropzone__hint {
    color: var(--p-text-disabled);
  }
  .dropzone--tile .dropzone__area :global(.p-button) {
    margin-top: 4px;
  }
</style>
