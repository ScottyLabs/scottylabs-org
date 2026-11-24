<script lang="ts">
  // A photo on its way up: a local preview, a progress bar and a way to cancel.
  import Button from "../portal/ui/Button.svelte";
  import type { UploadTask } from "./upload-task.svelte.ts";

  let {
    task,
    compact = false,
    oncancel,
  }: {
    task: UploadTask;
    /** For grid tiles: the preview fills the tile and the details sit under it. */
    compact?: boolean;
    oncancel: () => void;
  } = $props();

  const uid = $props.id();
  const labelId = `upload-${uid}-label`;
  const statusText = $derived(
    task.status === "processing" ? "Preparing sizes" : `Uploading, ${task.percent.toString()}%`,
  );

  let root: HTMLDivElement | undefined = $state();

  /** Focuses the Cancel upload button, for when the button that started the upload went away. */
  export function focus(): void {
    root?.querySelector("button")?.focus();
  }
</script>

<div bind:this={root} class={["upload", compact && "upload--compact"]}>
  <img class="upload__preview" src={task.preview} alt="" />
  <div class="upload__details">
    <span id={labelId} class="upload__name" title={task.file.name}>{task.file.name}</span>
    <div
      class="upload__bar"
      role="progressbar"
      aria-labelledby={labelId}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={task.status === "processing" ? undefined : task.percent}
      aria-valuetext={statusText}
    >
      <span
        class={["upload__fill", task.status === "processing" && "upload__fill--busy"]}
        style:width={`${task.percent.toString()}%`}
      ></span>
    </div>
    <div class="upload__footer">
      <span class="upload__status">{statusText}</span>
      <Button variant="subtle" size="sm" onclick={oncancel}>Cancel upload</Button>
    </div>
  </div>
</div>

<style>
  .upload {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 10px;
    border: 1px solid var(--p-border-muted);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .upload--compact {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
  }
  .upload__preview {
    flex: none;
    width: 72px;
    height: 54px;
    border-radius: var(--p-radius-sm);
    object-fit: cover;
    opacity: 0.6;
  }
  .upload--compact .upload__preview {
    width: 100%;
    height: auto;
    aspect-ratio: 4 / 3;
  }
  .upload__details {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .upload__name {
    overflow: hidden;
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .upload__bar {
    overflow: hidden;
    height: 4px;
    border-radius: var(--p-radius-pill);
    background: var(--p-surface-strong);
  }
  .upload__fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--p-focus);
    transition: width 200ms linear;
  }
  .upload__fill--busy {
    animation: upload-pulse 1.2s ease-in-out infinite;
  }
  .upload__footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 4px 8px;
  }
  .upload__status {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .upload--compact .upload__footer :global(.p-button) {
    padding: 0 8px;
  }

  @keyframes upload-pulse {
    50% {
      opacity: 0.5;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .upload__fill {
      transition: none;
    }
    .upload__fill--busy {
      animation: none;
    }
  }
</style>
