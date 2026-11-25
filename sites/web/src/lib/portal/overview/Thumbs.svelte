<script lang="ts">
  // A row of small covers on an overview card. A missing cover shows the committee tint, so the
  // row keeps its length and nothing shifts when images arrive.
  import type { ImageRef } from "../../api/types.ts";
  import { Img } from "../../media/index.ts";

  let {
    items,
    wide = false,
  }: {
    items: readonly { key: string; image: ImageRef | null; tint?: string }[];
    /** One cover in focus, three times as wide. */
    wide?: boolean;
  } = $props();
</script>

<!-- The card's headline says what these are; the pictures only decorate it. -->
<span class={["thumbs", wide && "thumbs--wide"]} aria-hidden="true">
  {#each items as item (item.key)}
    <span class="thumbs__item" style:background={item.tint}>
      {#if item.image !== null}
        <Img image={item.image} alt="" sizes={wide ? "120px" : "40px"} fallbackWidth={480} fit="cover" />
      {/if}
    </span>
  {/each}
</span>

<style>
  .thumbs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .thumbs__item {
    box-sizing: border-box;
    width: 40px;
    height: 28px;
    overflow: hidden;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 6px;
    background: var(--p-surface-muted);

    & > :global(img) {
      width: 100%;
      height: 100%;
    }
  }
  .thumbs--wide .thumbs__item {
    width: 120px;
  }
</style>
