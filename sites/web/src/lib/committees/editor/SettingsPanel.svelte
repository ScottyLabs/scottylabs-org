<script lang="ts">
  // The right panel: the selected block's settings, the page settings when the cover or nothing is
  // selected, and the phone order while the canvas shows the phone width (Figma 109:9826).
  import type { Block } from "../../api/types.ts";
  import { blockName } from "../blocks.ts";
  import BlockSettings from "./BlockSettings.svelte";
  import PageSettings from "./PageSettings.svelte";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    device,
    onremove,
  }: {
    editor: PageEditor;
    device: "desktop" | "phone";
    onremove: (block: Block) => void;
  } = $props();

  const selected = $derived(editor.selected);
</script>

<!-- A read-only page keeps its settings visible but switches every control off. -->
<fieldset class="panel" disabled={!editor.writable}>
  <legend class="p-sr-only">Settings</legend>
  {#if device === "phone" && selected === null}
    <div class="phone">
      <h2 class="phone__title">Phone preview</h2>
      <p class="phone__text">On phones every block is full width, in the order of the page outline.</p>
      <h3 class="phone__heading">Order on phones</h3>
      <ol class="phone__order">
        {#each editor.doc.blocks as block, index (block.id)}
          <li><span class="phone__number">{index + 1}</span>{blockName(block)}</li>
        {/each}
      </ol>
      <p class="phone__text">Nothing to set up. Block widths apply only on larger screens.</p>
    </div>
  {:else if selected === null || selected.type === "cover"}
    <PageSettings {editor} />
  {:else}
    {#key selected.id}
      <BlockSettings {editor} block={selected} {onremove} onpagesettings={() => editor.select(null)} />
    {/key}
  {/if}
</fieldset>

<style>
  .panel {
    box-sizing: border-box;
    min-width: 0;
    margin: 0;
    padding: 24px 20px 48px;
    border: 0;
  }
  .phone {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .phone__title {
    margin: 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .phone__heading {
    margin: 8px 0 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .phone__text {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .phone__order {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: flex;
      gap: 12px;
      color: var(--p-text);
      font-size: 0.9375rem;
      line-height: 24px;
    }
  }
  .phone__number {
    width: 20px;
    color: var(--p-text-tertiary);
    font-size: 0.8125rem;
    text-align: right;
  }
</style>
