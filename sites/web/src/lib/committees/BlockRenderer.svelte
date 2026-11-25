<script lang="ts" module>
  import type { Snippet } from "svelte";
  import type { Block } from "../api/types.ts";

  /** What the editor gets to wrap each block in, for its selection box and toolbar. */
  export interface BlockFrame {
    block: Block;
    index: number;
    /** Renders the block itself. */
    content: Snippet;
  }
</script>

<script lang="ts">
  // A committee page's blocks on the 12-column grid (spec C-6 and C-10), shared by the public page
  // and the editor canvas. Rows fill left to right with 24 px gutters and wrap when a block does
  // not fit; below 640 px of width every block is full width, in the same order. The breakpoints
  // are container queries, so the editor's phone preview lays out like a phone.
  //
  //   <BlockRenderer {blocks} {committee} {settings} {projects} />
  //
  // In the editor, `mode="editor"` shows placeholders for empty fields and keeps links from
  // navigating, `editableId` makes one block's text editable in place, `frame` wraps each block,
  // and `freeSlot` fills rows that have columns to spare.
  import "./committees.css";
  import type { CommitteeSettings, ProjectCard } from "../api/types.ts";
  import { freeSlots, type FreeSlot } from "./layout.ts";
  import CoverBlock from "./render/CoverBlock.svelte";
  import CtaBlock from "./render/CtaBlock.svelte";
  import GalleryBlock from "./render/GalleryBlock.svelte";
  import ImageBlock from "./render/ImageBlock.svelte";
  import ProjectsBlock from "./render/ProjectsBlock.svelte";
  import QuoteBlock from "./render/QuoteBlock.svelte";
  import StatsBlock from "./render/StatsBlock.svelte";
  import TeamsBlock from "./render/TeamsBlock.svelte";
  import TextBlock from "./render/TextBlock.svelte";
  import VideoBlock from "./render/VideoBlock.svelte";
  import WorkSessionsBlock from "./render/WorkSessionsBlock.svelte";
  import type { CommitteeInfo, RenderMode } from "./render/types.ts";

  let {
    blocks,
    committee,
    settings,
    projects,
    mode = "view",
    editableId = null,
    onblockchange,
    frame,
    freeSlot,
    class: className,
  }: {
    blocks: readonly Block[];
    committee: CommitteeInfo;
    settings: CommitteeSettings;
    projects: readonly ProjectCard[];
    mode?: RenderMode;
    /** The block whose text can be edited in place. */
    editableId?: string | null;
    onblockchange?: (block: Block) => void;
    frame?: Snippet<[BlockFrame]>;
    /** Shown in the free columns at the end of a row. */
    freeSlot?: Snippet<[FreeSlot]>;
    class?: string;
  } = $props();

  const editor = $derived(mode === "editor");
  const slots = $derived(freeSlot === undefined ? [] : freeSlots(blocks));

  function slotAfter(index: number): FreeSlot | undefined {
    return slots.find((slot) => slot.afterIndex === index);
  }

  function change(block: Block): void {
    onblockchange?.(block);
  }
</script>

{#snippet content(block: Block)}
  {@const editable = editor && editableId === block.id}
  {#if block.type === "cover"}
    <CoverBlock
      props={block.props}
      {committee}
      {settings}
      {editor}
      {editable}
      onchange={(props) => change({ ...block, props })}
    />
  {:else if block.type === "text"}
    <TextBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "image"}
    <ImageBlock
      props={block.props}
      width={block.width}
      {editor}
      {editable}
      onchange={(props) => change({ ...block, props })}
    />
  {:else if block.type === "gallery"}
    <GalleryBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "video"}
    <VideoBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "stats"}
    <StatsBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "teams"}
    <TeamsBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "projects"}
    <ProjectsBlock
      props={block.props}
      {committee}
      {projects}
      {editor}
      {editable}
      onchange={(props) => change({ ...block, props })}
    />
  {:else if block.type === "quote"}
    <QuoteBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {:else if block.type === "work_sessions"}
    <WorkSessionsBlock {settings} {editor} />
  {:else if block.type === "cta"}
    <CtaBlock props={block.props} {editor} {editable} onchange={(props) => change({ ...block, props })} />
  {/if}
{/snippet}

<div
  class={["cb-scope", "cb-page", className]}
  style:--cb-accent={committee.color}
  style:--cb-tint={committee.tint}
>
  <div class="cb-grid">
    {#each blocks as block, index (block.id)}
      {@const slot = slotAfter(index)}
      <div
        class={["cb-cell", `cb-cell--${block.type}`]}
        style:--span={block.width}
        data-block-id={block.id}
      >
        {#snippet blockContent()}
          {@render content(block)}
        {/snippet}
        {#if frame}
          {@render frame({ block, index, content: blockContent })}
        {:else}
          {@render blockContent()}
        {/if}
      </div>
      {#if slot && freeSlot}
        <div class="cb-cell cb-cell--free" style:--span={slot.columns}>
          {@render freeSlot(slot)}
        </div>
      {/if}
    {/each}
  </div>
</div>

<style>
  .cb-page {
    container: committee-page / inline-size;
  }
  .cb-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    column-gap: 24px;
    row-gap: 72px;
  }
  /* Each block is a container too, so narrow blocks can tighten their own type. */
  .cb-cell {
    grid-column: span var(--span, 12);
    align-self: start;
    min-width: 0;
    container: block / inline-size;
  }
  .cb-cell--quote {
    align-self: center;
  }
  .cb-cell--free {
    align-self: stretch;
  }

  @container committee-page (width < 640px) {
    .cb-grid {
      grid-template-columns: minmax(0, 1fr);
      row-gap: 48px;
    }
    .cb-cell {
      grid-column: auto;
    }
    .cb-cell--free {
      display: none;
    }
  }
</style>
