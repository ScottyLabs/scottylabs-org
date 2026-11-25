<script lang="ts">
  // "Page outline": every block in order. Choose one to select it; drag the handle, or press Alt
  // with the up and down arrows, to move it. The cover stays at the top.
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Lock from "@lucide/svelte/icons/lock";
  import { blockName } from "../blocks.ts";
  import { BLOCK_ICONS } from "./icons.ts";
  import { sortableHandle } from "./sortable.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    onmoved,
    onselect,
  }: {
    editor: PageEditor;
    onmoved: (id: string) => void;
    /** Selects a block and shows it on the canvas. */
    onselect: (id: string) => void;
  } = $props();

  const uid = $props.id();
  let dragKey: string | null = null;

  function onkeydown(event: KeyboardEvent, id: string) {
    if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    if (editor.moveBy(id, event.key === "ArrowUp" ? -1 : 1)) onmoved(id);
  }
</script>

<section class="outline" aria-labelledby={`${uid}-title`}>
  <h2 id={`${uid}-title`} class="outline__title">Page outline</h2>
  <p id={`${uid}-hint`} class="p-sr-only">
    Choose a block to select it. Press Alt with the up or down arrow to move it.
  </p>
  <ol class="outline__list">
    {#each editor.doc.blocks as block, index (block.id)}
      {@const Icon = BLOCK_ICONS[block.type]}
      {@const cover = block.type === "cover"}
      {@const flagged = editor.problems.blocks[block.id] !== undefined}
      <li class={["outline__item", editor.selectedId === block.id && "outline__item--selected"]} data-sortable-item>
        {#if cover || !editor.writable}
          <span class="outline__handle outline__handle--locked" aria-hidden="true">
            {#if cover}<Lock size={14} strokeWidth={2} />{/if}
          </span>
        {:else}
          <span
            class="outline__handle"
            title="Drag to move"
            aria-hidden="true"
            {@attach sortableHandle({
              index: () => index,
              onstart: () => {
                dragKey = `outline-drag-${block.id}-${Date.now()}`;
                editor.select(block.id);
              },
              onmove: (_from, to) => editor.moveBlock(block.id, to, dragKey),
              onend: () => onmoved(block.id),
            })}
          >
            <GripVertical size={16} strokeWidth={1.75} />
          </span>
        {/if}
        <button
          type="button"
          class="outline__button"
          aria-current={editor.selectedId === block.id ? "true" : undefined}
          aria-describedby={cover ? undefined : `${uid}-hint`}
          onclick={() => onselect(block.id)}
          onkeydown={(event) => (cover || !editor.writable ? undefined : onkeydown(event, block.id))}
        >
          <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
          <span class="outline__name">{blockName(block)}</span>
          {#if flagged}
            <CircleAlert class="outline__flag" size={16} strokeWidth={2} aria-hidden="true" />
            <span class="p-sr-only">, needs attention</span>
          {/if}
        </button>
      </li>
    {/each}
  </ol>
</section>

<style>
  .outline__title {
    margin: 0 0 10px;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .outline__list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .outline__item {
    display: flex;
    align-items: center;
    border-radius: var(--p-radius-sm);

    &:hover {
      background: var(--p-surface-muted);
    }
  }
  .outline__item--selected,
  .outline__item--selected:hover {
    background: var(--p-info-surface);
  }
  .outline__handle {
    display: grid;
    flex: none;
    place-items: center;
    width: 24px;
    height: 32px;
    color: var(--p-text-tertiary);
    cursor: grab;
    touch-action: none;
  }
  .outline__handle--locked {
    cursor: default;
  }
  .outline__button {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 6px 8px 6px 2px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text);
    font-size: 0.9375rem;
    line-height: 20px;
    cursor: pointer;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .outline__name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .outline__button :global(.outline__flag) {
    flex: none;
    color: var(--p-danger-text);
  }
</style>
