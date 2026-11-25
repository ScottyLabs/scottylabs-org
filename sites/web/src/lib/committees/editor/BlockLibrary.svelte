<script lang="ts">
  // "Add a block": drag a tile onto the page, or click it to add the block at the end.
  import { type AddableType, BLOCK_LABELS, LIBRARY, MAX_BLOCKS } from "../blocks.ts";
  import { DRAG_TYPE, type EditorDrag } from "./drag.ts";
  import { BLOCK_ICONS } from "./icons.ts";

  let {
    disabled,
    full,
    onadd,
    ondragchange,
  }: {
    disabled: boolean;
    /** The page has the most blocks it may have. */
    full: boolean;
    onadd: (type: AddableType) => void;
    ondragchange: (drag: EditorDrag | null) => void;
  } = $props();

  const uid = $props.id();
  const blocked = $derived(disabled || full);

  function ondragstart(event: DragEvent, type: AddableType) {
    if (event.dataTransfer === null) return;
    event.dataTransfer.setData(DRAG_TYPE, type);
    event.dataTransfer.effectAllowed = "copy";
    ondragchange({ kind: "new", type });
  }
</script>

<section class="library" aria-labelledby={`${uid}-title`}>
  <h2 id={`${uid}-title`} class="library__title">Add a block</h2>
  <p class="library__hint" id={`${uid}-hint`}>
    {#if full}
      This page has {MAX_BLOCKS} blocks, the most it can have. Remove one to add another.
    {:else}
      Drag one onto the page, or click it to add it at the end.
    {/if}
  </p>
  <ul class="library__tiles">
    {#each LIBRARY as type (type)}
      {@const Icon = BLOCK_ICONS[type]}
      <li>
        <button
          type="button"
          class="library__tile"
          draggable={!blocked}
          disabled={blocked}
          aria-describedby={`${uid}-hint`}
          onclick={() => onadd(type)}
          ondragstart={(event) => ondragstart(event, type)}
          ondragend={() => ondragchange(null)}
        >
          <Icon size={20} strokeWidth={1.75} aria-hidden="true" />
          <span>{BLOCK_LABELS[type]}</span>
        </button>
      </li>
    {/each}
  </ul>
</section>

<style>
  .library__title {
    margin: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .library__hint {
    margin: 4px 0 16px;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .library__tiles {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: block;
    }
  }
  .library__tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    box-sizing: border-box;
    width: 100%;
    min-height: 58px;
    padding: 8px 6px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font-size: 0.8125rem;
    line-height: 18px;
    text-align: center;
    cursor: grab;
    transition:
      border-color var(--p-duration) var(--p-ease),
      background-color var(--p-duration) var(--p-ease);

    &:hover:not(:disabled) {
      border-color: var(--p-selection);
      background: var(--p-info-surface);
    }
    &:active:not(:disabled) {
      cursor: grabbing;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: not-allowed;
    }
  }
</style>
