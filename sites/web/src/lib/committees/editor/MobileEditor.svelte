<script lang="ts">
  // The editor on phones (Figma 112:10467, spec CP-10): the blocks as a list to reorder and open,
  // with widths left to larger screens. Reorder by dragging a handle or with Move up and Move down.
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Lock from "@lucide/svelte/icons/lock";
  import Plus from "@lucide/svelte/icons/plus";
  import type { Block } from "../../api/types.ts";
  import { Button, IconButton, Sheet } from "../../portal/ui/index.ts";
  import { type AddableType, BLOCK_DESCRIPTIONS, BLOCK_LABELS, LIBRARY, MAX_BLOCKS, blockName } from "../blocks.ts";
  import BlockSettings from "./BlockSettings.svelte";
  import PageSettings from "./PageSettings.svelte";
  import { BLOCK_ICONS } from "./icons.ts";
  import { sortableHandle } from "./sortable.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    saveText,
    onpreview,
    onpublish,
    onremove,
    onmoved,
  }: {
    editor: PageEditor;
    saveText: string;
    onpreview: () => void;
    onpublish: () => void;
    onremove: (block: Block) => void;
    onmoved: (id: string) => void;
  } = $props();

  const uid = $props.id();
  let adding = $state(false);
  let dragKey: string | null = null;

  function toggle(id: string) {
    editor.select(editor.selectedId === id ? null : id);
  }

  function add(type: AddableType) {
    adding = false;
    editor.addBlock(type);
  }

  function move(id: string, step: -1 | 1) {
    if (editor.moveBy(id, step)) onmoved(id);
  }
</script>

<div class="mobile">
  <header class="mobile__bar">
    <a class="mobile__back" href="/portal/committees/">
      <ChevronLeft size={20} strokeWidth={2} aria-hidden="true" />
      <span><span class="p-sr-only">Back to committee pages. Editing the </span>{editor.committee.name} page</span>
    </a>
    <button type="button" class="mobile__preview" onclick={onpreview}>Preview</button>
  </header>

  <div class="mobile__main">
    <h1 class="p-sr-only">{editor.committee.name} committee page</h1>
    {#if editor.writable}
      <p class="mobile__saved" role="status">{saveText}</p>
      <p class="mobile__note">
        Reorder and edit blocks here. Block widths apply on larger screens, so set those on a computer.
      </p>
    {/if}

    <ol class="mobile__list">
      {#each editor.doc.blocks as block, index (block.id)}
        {@const Icon = BLOCK_ICONS[block.type]}
        {@const open = editor.selectedId === block.id}
        {@const flagged = editor.problems.blocks[block.id] !== undefined}
        <li class={["mobile__card", open && "mobile__card--open", flagged && "mobile__card--flagged"]} data-sortable-item>
          <div class="mobile__row">
            {#if block.type === "cover" || !editor.writable}
              <span class="mobile__handle mobile__handle--static" aria-hidden="true">
                {#if block.type === "cover"}<Lock size={16} strokeWidth={2} />{/if}
              </span>
            {:else}
              <span
                class="mobile__handle"
                aria-hidden="true"
                {@attach sortableHandle({
                  index: () => index,
                  onstart: () => (dragKey = `mobile-drag-${block.id}-${Date.now()}`),
                  onmove: (_from, to) => editor.moveBlock(block.id, to, dragKey),
                  onend: (moved) => {
                    if (moved) onmoved(block.id);
                  },
                })}
              >
                <GripVertical size={18} strokeWidth={1.75} />
              </span>
            {/if}
            <button
              type="button"
              class="mobile__toggle"
              aria-expanded={open}
              aria-controls={`${uid}-${block.id}`}
              onclick={() => toggle(block.id)}
            >
              <Icon size={18} strokeWidth={1.75} aria-hidden="true" />
              <span class="mobile__name">{blockName(block)}</span>
              {#if flagged}
                <CircleAlert class="mobile__flag" size={16} strokeWidth={2} aria-hidden="true" /><span class="p-sr-only">, needs attention</span>
              {/if}
              {#if open}
                <ChevronDown size={18} strokeWidth={1.75} aria-hidden="true" />
              {:else}
                <ChevronRight size={18} strokeWidth={1.75} aria-hidden="true" />
              {/if}
            </button>
          </div>
          {#if open}
            <div class="mobile__body" id={`${uid}-${block.id}`}>
              {#if block.type === "cover"}
                <PageSettings {editor} compact />
              {:else}
                <BlockSettings {editor} {block} compact {onremove} onpagesettings={() => editor.select(editor.doc.blocks[0]?.id ?? null)} />
              {/if}
              <div class="mobile__actions">
                {#if block.type !== "cover" && editor.writable}
                  <IconButton label="Move up" icon={ArrowUp} disabled={index <= 1} onclick={() => move(block.id, -1)} />
                  <IconButton
                    label="Move down"
                    icon={ArrowDown}
                    disabled={index >= editor.doc.blocks.length - 1}
                    onclick={() => move(block.id, 1)}
                  />
                  <button type="button" class="mobile__remove" onclick={() => onremove(block)}>Remove block</button>
                {/if}
                <Button size="sm" onclick={() => editor.select(null)}>Done</Button>
              </div>
            </div>
          {/if}
        </li>
      {/each}
    </ol>

    {#if editor.writable}
      <button type="button" class="mobile__add" disabled={!editor.canAddBlock} onclick={() => (adding = true)}>
        <Plus size={18} strokeWidth={1.75} aria-hidden="true" />
        {editor.canAddBlock ? "Add a block" : `This page has ${MAX_BLOCKS} blocks, the most it can have`}
      </button>
    {/if}
  </div>

  {#if editor.writable}
    <div class="mobile__publish">
      <Button size="lg" fullWidth onclick={onpublish}>Publish page</Button>
    </div>
  {/if}
</div>

<Sheet bind:open={adding} title="Add a block" description="It goes at the end of the page. Move it from there.">
  <ul class="mobile__types">
    {#each LIBRARY as type (type)}
      {@const Icon = BLOCK_ICONS[type]}
      <li>
        <button type="button" class="mobile__type" onclick={() => add(type)}>
          <Icon size={20} strokeWidth={1.75} aria-hidden="true" />
          <span>
            <span class="mobile__type-name">{BLOCK_LABELS[type]}</span>
            <span class="mobile__type-description">{BLOCK_DESCRIPTIONS[type]}</span>
          </span>
        </button>
      </li>
    {/each}
  </ul>
</Sheet>

<style>
  .mobile {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    background: var(--p-bg);
  }
  .mobile__bar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 56px;
    padding: 0 12px;
    border-bottom: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .mobile__back {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--p-text);
    font-size: 1.125rem;
    font-weight: 500;
    line-height: 24px;
    text-decoration: none;

    &:focus-visible {
      border-radius: 4px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .mobile__preview {
    padding: 8px;
    color: var(--p-text-secondary);
    font-size: 0.9375rem;
    cursor: pointer;

    &:focus-visible {
      border-radius: 4px;
      outline: 2px solid var(--p-focus);
    }
  }
  .mobile__main {
    flex: 1;
    padding: 16px 20px 120px;
  }
  .mobile__saved {
    margin: 0 0 8px;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
  }
  .mobile__note {
    margin: 0 0 12px;
    padding: 10px 14px;
    border-radius: var(--p-radius);
    background: var(--p-surface-strong);
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .mobile__list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .mobile__card {
    display: block;
    border: 1px solid var(--p-border);
    border-radius: 12px;
    background: var(--p-surface);
  }
  .mobile__card--open {
    border: 2px solid var(--p-selection);
  }
  .mobile__card--flagged {
    border-color: var(--p-danger);
  }
  .mobile__row {
    display: flex;
    align-items: center;
  }
  .mobile__handle {
    display: grid;
    flex: none;
    place-items: center;
    width: 40px;
    height: 44px;
    color: var(--p-text-secondary);
    touch-action: none;
    cursor: grab;
  }
  .mobile__handle--static {
    cursor: default;
  }
  .mobile__toggle {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    min-height: 44px;
    padding: 0 14px 0 0;
    color: var(--p-text);
    font-size: 0.9375rem;
    cursor: pointer;

    &:focus-visible {
      border-radius: 8px;
      outline: 2px solid var(--p-focus);
    }
  }
  .mobile__name {
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mobile__toggle :global(.mobile__flag) {
    color: var(--p-danger-text);
  }
  .mobile__body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 4px 14px 14px;
  }
  .mobile__actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }
  .mobile__remove {
    margin-left: auto;
    padding: 8px 10px;
    color: var(--p-text);
    font-size: 0.875rem;
    cursor: pointer;

    &:focus-visible {
      border-radius: 4px;
      outline: 2px solid var(--p-focus);
    }
  }
  .mobile__add {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    box-sizing: border-box;
    margin-top: 10px;
    padding: 12px;
    border: 1px dashed #b8b8b8;
    border-radius: 12px;
    color: var(--p-text);
    font-size: 0.9375rem;
    cursor: pointer;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
    }
    &:disabled {
      color: var(--p-text-secondary);
      cursor: not-allowed;
    }
  }
  .mobile__publish {
    position: fixed;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 2;
    padding: 12px 20px calc(12px + env(safe-area-inset-bottom));
    border-top: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .mobile__types {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: block;
    }
  }
  .mobile__type {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    width: 100%;
    box-sizing: border-box;
    padding: 10px;
    border-radius: var(--p-radius);
    color: var(--p-text);
    text-align: left;
    cursor: pointer;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
    }
    & > :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
  .mobile__type-name {
    display: block;
    font-size: 0.9375rem;
    font-weight: 500;
    line-height: 22px;
  }
  .mobile__type-description {
    display: block;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
</style>
