<script lang="ts">
  // The page as it will look, with editing on top (Figma 109:7297 to 109:9177): select a block to
  // get the blue box, its label and toolbar; drag it, or a tile from the library, onto the blue
  // drop line or into a row's free columns, where it fills them (109:8655); add a block in free
  // columns from their menu; edit text in place. The 12-column grid shows while dragging or when
  // the grid is on.
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Copy from "@lucide/svelte/icons/copy";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash";
  import type { Block, BlockWidth } from "../../api/types.ts";
  import RichTextEditor from "../../editor/RichTextEditor.svelte";
  import { Menu, type MenuItem } from "../../portal/ui/index.ts";
  import SelectionBox from "../../site/SelectionBox.svelte";
  import BlockRenderer, { type BlockFrame } from "../BlockRenderer.svelte";
  import { BLOCK_LABELS, LIBRARY, MAX_BLOCKS, WIDTH_LABELS, blockName } from "../blocks.ts";
  import { type FreeSlot, dropIndex, freeSlots, placeInSlot, slotWidth } from "../layout.ts";
  import { DRAG_TYPE, type EditorDrag } from "./drag.ts";
  import { type DropLine, dropLineAt } from "./drop.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    device,
    showGrid,
    drag = $bindable(null),
    onmoved,
    onremoved,
  }: {
    editor: PageEditor;
    device: "desktop" | "phone";
    showGrid: boolean;
    /** What is being dragged, shared with the block library. */
    drag?: EditorDrag | null;
    onmoved: (id: string) => void;
    /** After a block was removed; `index` is where it was. */
    onremoved: (block: Block, index: number) => void;
  } = $props();

  let sheet: HTMLElement | undefined = $state();
  let line = $state<DropLine | null>(null);
  /** The free columns under the pointer while dragging, and the width the block would get there. */
  let slotTarget = $state<{ slot: FreeSlot; width: BlockWidth } | null>(null);
  const settings = $derived(editor.doc.settings);
  const gridVisible = $derived((showGrid || drag !== null) && device === "desktop");

  // -------------------------------------------------------------------------
  // Dragging

  function layout() {
    if (sheet === undefined) return null;
    const grid = sheet.querySelector<HTMLElement>(".cb-grid");
    if (grid === null) return null;
    const origin = sheet.getBoundingClientRect();
    const relative = (rect: DOMRect) => ({
      left: rect.left - origin.left,
      top: rect.top - origin.top,
      right: rect.right - origin.left,
      bottom: rect.bottom - origin.top,
    });
    const style = getComputedStyle(grid);
    const blocks = [...grid.querySelectorAll<HTMLElement>(":scope > [data-block-id]")].map((cell) =>
      relative(cell.getBoundingClientRect()),
    );
    return {
      layout: {
        blocks,
        grid: relative(grid.getBoundingClientRect()),
        rowGap: pixels(style.rowGap),
        columnGap: pixels(style.columnGap),
      },
      origin,
    };
  }

  /** "72px" as 72. */
  function pixels(value: string): number {
    const number = Number(value.replace(/px$/u, ""));
    return Number.isFinite(number) ? number : 0;
  }

  /** The free columns under the pointer, when the dragged block can go there. */
  function slotUnder(event: DragEvent, dragged: EditorDrag): { slot: FreeSlot; width: BlockWidth } | null {
    const cell = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-free-slot]") : null;
    if (cell === null) return null;
    const slot = freeSlots(editor.doc.blocks).find((candidate) => String(candidate.afterIndex) === cell.dataset.freeSlot);
    if (slot === undefined) return null;
    if (dragged.kind === "new") {
      const width = slotWidth(dragged.type, slot);
      return width === null || !editor.canAddBlock ? null : { slot, width };
    }
    const placed = placeInSlot(editor.doc.blocks, dragged.id, slot)?.find((block) => block.id === dragged.id);
    return placed === undefined ? null : { slot, width: placed.width };
  }

  function ondragover(event: DragEvent) {
    if (drag === null || !editor.writable) return;
    event.preventDefault();
    if (event.dataTransfer !== null) event.dataTransfer.dropEffect = drag.kind === "new" ? "copy" : "move";
    slotTarget = slotUnder(event, drag);
    if (slotTarget !== null) {
      line = null;
      return;
    }
    const measured = layout();
    if (measured === null) return;
    line = dropLineAt(
      measured.layout,
      event.clientX - measured.origin.left,
      event.clientY - measured.origin.top,
    );
  }

  function ondragleave(event: DragEvent) {
    if (sheet !== undefined && event.relatedTarget instanceof Node && sheet.contains(event.relatedTarget)) return;
    line = null;
    slotTarget = null;
  }

  function ondrop(event: DragEvent) {
    event.preventDefault();
    const target = line;
    const slot = slotTarget;
    const dragged = drag;
    line = null;
    slotTarget = null;
    drag = null;
    if (dragged === null) return;
    if (slot !== null) {
      dropInSlot(dragged, slot.slot, slot.width);
      return;
    }
    if (target === null) return;
    if (dragged.kind === "new") {
      editor.addBlock(dragged.type, target.before);
      return;
    }
    const from = editor.doc.blocks.findIndex((block) => block.id === dragged.id);
    if (from > 0 && editor.moveBlock(dragged.id, dropIndex(from, target.before))) onmoved(dragged.id);
  }

  function dropInSlot(dragged: EditorDrag, slot: FreeSlot, width: BlockWidth) {
    if (dragged.kind === "new") editor.addBlock(dragged.type, slot.afterIndex + 1, width);
    else if (editor.moveIntoSlot(dragged.id, slot)) onmoved(dragged.id);
  }

  function startMove(event: DragEvent, block: Block) {
    if (event.dataTransfer === null) return;
    event.dataTransfer.setData(DRAG_TYPE, block.id);
    event.dataTransfer.effectAllowed = "move";
    const cell = sheet?.querySelector(`[data-block-id="${CSS.escape(block.id)}"]`);
    if (cell instanceof HTMLElement) event.dataTransfer.setDragImage(cell, 24, 24);
    drag = { kind: "move", id: block.id };
  }

  function endDrag() {
    drag = null;
    line = null;
    slotTarget = null;
  }

  // -------------------------------------------------------------------------
  // Selecting and the toolbar

  function remove(block: Block) {
    const index = editor.doc.blocks.findIndex((candidate) => candidate.id === block.id);
    if (editor.remove(block.id)) onremoved(block, index);
  }

  function move(block: Block, step: -1 | 1) {
    if (editor.moveBy(block.id, step)) onmoved(block.id);
  }

  // Links on the canvas never navigate away from the editor.
  function holdLinks(event: MouseEvent) {
    if (event.target instanceof Element && event.target.closest("a") !== null) event.preventDefault();
  }

  // -------------------------------------------------------------------------
  // Free columns

  function slotItems(slot: FreeSlot): MenuItem[] {
    const full = editor.doc.blocks.length >= MAX_BLOCKS;
    return LIBRARY.flatMap((type) => {
      const width = slotWidth(type, slot);
      if (width === null) return [];
      return [
        {
          label: `${BLOCK_LABELS[type]}, ${WIDTH_LABELS[width].toLowerCase()} width`,
          disabled: full,
          onSelect: () => editor.addBlock(type, slot.afterIndex + 1, width),
        },
      ];
    });
  }
</script>

{#snippet frame({ block, index, content }: BlockFrame)}
  {@const selected = editor.selectedId === block.id}
  {@const flagged = editor.problems.blocks[block.id] !== undefined}
  {@const cover = block.type === "cover"}
  <!-- Pointer selection only: keyboard users select blocks in the page outline, and Tab then
       reaches the selected block's toolbar and text, which are the only stops on the canvas. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class={["frame", "sl-select-host", selected && "frame--selected", flagged && "frame--flagged", drag?.kind === "move" && drag.id === block.id && "frame--dragging"]}
    onpointerdown={() => {
      if (!selected) editor.select(block.id);
    }}
    onclickcapture={holdLinks}
  >
    {@render content()}
    {#if flagged}
      <span class="frame__flag"><CircleAlert size={14} strokeWidth={2} aria-hidden="true" />Needs attention</span>
    {/if}
    {#if selected}
      <SelectionBox active inset={-8} />
      <span class="frame__tab" aria-hidden="true">
        <GripVertical size={12} strokeWidth={2} />{BLOCK_LABELS[block.type]}
      </span>
      {#if editor.writable && !cover}
        <div class="frame__toolbar" role="group" aria-label={`${blockName(block)} block, ${index + 1} of ${editor.doc.blocks.length}`}>
          <span
            class="frame__tool frame__tool--drag"
            draggable="true"
            title="Drag to move"
            aria-hidden="true"
            ondragstart={(event) => startMove(event, block)}
            ondragend={endDrag}
          >
            <GripVertical size={16} strokeWidth={1.75} />
          </span>
          <button type="button" class="frame__tool" aria-label="Move up" title="Move up" disabled={index <= 1} onclick={() => move(block, -1)}>
            <ArrowUp size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="frame__tool"
            aria-label="Move down"
            title="Move down"
            disabled={index >= editor.doc.blocks.length - 1}
            onclick={() => move(block, 1)}
          >
            <ArrowDown size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
          <button
            type="button"
            class="frame__tool"
            aria-label="Duplicate"
            title="Duplicate"
            disabled={!editor.canAddBlock}
            onclick={() => editor.duplicate(block.id)}
          >
            <Copy size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
          <button type="button" class="frame__tool" aria-label="Remove block" title="Remove block" onclick={() => remove(block)}>
            <Trash size={16} strokeWidth={1.75} aria-hidden="true" />
          </button>
        </div>
      {/if}
    {/if}
  </div>
{/snippet}

{#snippet freeSlot(slot: FreeSlot)}
  {@const target = slotTarget?.slot.afterIndex === slot.afterIndex ? slotTarget : null}
  <Menu class="canvas__slot-menu" label="Add a block here" items={slotItems(slot)} align="start" width={260}>
    {#snippet trigger(props)}
      <button {...props} type="button" class={["slot", target && "slot--target"]} disabled={!editor.canAddBlock}>
        <Plus size={18} strokeWidth={1.75} aria-hidden="true" />
        {#if target}
          <span class="slot__label">Drop here</span>
          <span class="slot__free">{WIDTH_LABELS[target.width]} width</span>
        {:else}
          <span class="slot__label">Add a block here</span>
          <span class="slot__free">{slot.columns} of 12 columns free</span>
        {/if}
      </button>
    {/snippet}
  </Menu>
{/snippet}

<!-- A landmark, so the page's own headings read as grouped under it. -->
<div class={["canvas", `canvas--${device}`]} role="region" aria-label="Page canvas">
  <div
    bind:this={sheet}
    class="canvas__sheet"
    role="presentation"
    {ondragover}
    {ondragleave}
    {ondrop}
    onpointerdown={(event) => {
      if (event.target === event.currentTarget) editor.select(null);
    }}
  >
    {#if gridVisible}
      <div class="canvas__grid" aria-hidden="true">
        {#each { length: 12 } as _, column (column)}<span></span>{/each}
      </div>
    {/if}
    <BlockRenderer
      blocks={editor.doc.blocks}
      committee={editor.committee}
      {settings}
      projects={editor.projects}
      mode="editor"
      editableId={editor.writable ? editor.selectedId : null}
      onblockchange={(block) => editor.updateBlock(block, `text:${block.id}`)}
      {frame}
      freeSlot={editor.writable && device === "desktop" ? freeSlot : undefined}
      textEditor={RichTextEditor}
    />
    {#if line}
      <span
        class={["canvas__line", line.vertical && "canvas__line--vertical"]}
        style:left={`${line.x}px`}
        style:top={`${line.y}px`}
        style:--length={`${line.length}px`}
        aria-hidden="true"
      ></span>
    {/if}
  </div>
  {#if drag !== null}
    <p class="canvas__hint" role="status">Drop on the blue line or in free columns. Blocks snap to rows on the 12-column grid.</p>
  {/if}
</div>

<style>
  .canvas {
    position: relative;
    box-sizing: border-box;
    min-height: 100%;
    padding: 16px 36px 96px;
  }
  .canvas__sheet {
    position: relative;
    box-sizing: border-box;
    width: 100%;
    max-width: 1280px;
    margin: 0 auto;
    padding: 36px 32px 64px;
    border: 1px solid var(--p-border);
    border-radius: 12px;
    background: #ffffff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  }
  .canvas--phone .canvas__sheet {
    width: 390px;
    padding: 24px 20px 48px;
    border-radius: 32px;
  }
  .canvas__grid {
    position: absolute;
    inset: 0 32px;
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 24px;
    pointer-events: none;

    & span {
      background: rgba(36, 164, 255, 0.07);
    }
  }
  .canvas__line {
    position: absolute;
    z-index: 5;
    width: var(--length);
    height: 3px;
    border-radius: 2px;
    background: var(--p-selection);
    transform: translateY(-1px);
    pointer-events: none;

    &::before,
    &::after {
      position: absolute;
      top: -4px;
      width: 7px;
      height: 7px;
      border: 2px solid var(--p-selection);
      border-radius: 50%;
      background: #ffffff;
      content: "";
    }
    &::before {
      left: -6px;
    }
    &::after {
      right: -6px;
    }
  }
  .canvas__line--vertical {
    width: 3px;
    height: var(--length);
    transform: translateX(-1px);

    &::before,
    &::after {
      left: -4px;
      right: auto;
    }
    &::before {
      top: -6px;
    }
    &::after {
      top: auto;
      bottom: -6px;
    }
  }
  .canvas__hint {
    position: sticky;
    bottom: 24px;
    z-index: 6;
    width: max-content;
    max-width: calc(100% - 32px);
    margin: 16px auto 0;
    padding: 10px 16px;
    border-radius: var(--p-radius);
    background: var(--p-toast);
    color: #ffffff;
    font-size: 0.875rem;
    line-height: 20px;
  }

  .frame {
    position: relative;
    height: 100%;
    border-radius: 4px;
    outline: 1px dashed transparent;
    outline-offset: 7px;
    cursor: default;
    transition: outline-color var(--p-duration) var(--p-ease);

    &:hover {
      outline-color: rgba(36, 164, 255, 0.5);
    }
  }
  .frame--selected:hover {
    outline-color: transparent;
  }
  .frame--flagged {
    outline: 2px solid var(--p-danger);
  }
  .frame--dragging {
    opacity: 0.4;
  }
  .frame__flag {
    position: absolute;
    top: -34px;
    right: -8px;
    z-index: 4;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 4px;
    background: var(--p-danger);
    color: #ffffff;
    font-size: 0.75rem;
    line-height: 18px;
  }
  .frame--selected .frame__flag {
    top: auto;
    bottom: -34px;
  }
  .frame__tab {
    position: absolute;
    bottom: calc(100% + 8px);
    left: -10px;
    z-index: 4;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 4px 4px 0 0;
    background: var(--p-selection);
    color: #ffffff;
    font-family: var(--p-mono);
    font-size: 0.6875rem;
    line-height: 16px;
    white-space: nowrap;
    pointer-events: none;
  }
  .frame__toolbar {
    position: absolute;
    bottom: calc(100% + 6px);
    right: -8px;
    z-index: 4;
    display: flex;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--p-border);
    border-radius: 10px;
    background: #ffffff;
    box-shadow: var(--p-shadow-pop);
  }
  .frame__tool {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--p-text);
    cursor: pointer;

    &:hover:not(:disabled) {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: not-allowed;
    }
  }
  .frame__tool--drag {
    cursor: grab;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-height: 140px;
    border: 1px dashed var(--p-selection);
    border-radius: 12px;
    background: rgba(36, 164, 255, 0.06);
    color: var(--p-text);
    cursor: pointer;

    &:hover:not(:disabled) {
      background: rgba(36, 164, 255, 0.12);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      display: none;
    }
  }
  /* A full page hides the add button, but a block moved from elsewhere can still go here. */
  .slot--target,
  .slot--target:disabled,
  .slot--target:hover:not(:disabled) {
    display: flex;
    border-style: solid;
    background: rgba(36, 164, 255, 0.16);
    box-shadow: inset 0 0 0 1px var(--p-selection);
  }
  .slot__label {
    font-size: 0.9375rem;
    line-height: 22px;
  }
  .slot__free {
    color: var(--p-text-secondary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .canvas :global(.canvas__slot-menu) {
    display: flex;
    width: 100%;
    height: 100%;
  }

  @media (prefers-reduced-motion: reduce) {
    .frame {
      transition: none;
    }
  }
</style>
