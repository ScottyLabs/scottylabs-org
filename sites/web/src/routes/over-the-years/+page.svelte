<script lang="ts">
  import { HISTORY, type HistoryBlock } from "#lib/data/history.ts";
  import Article from "#lib/sections/history/Article.svelte";
  import Carousel from "#lib/sections/history/Carousel.svelte";

  type YearGroup = {
    year: string;
    blocks: HistoryBlock[];
  };

  const groupByYear = (blocks: HistoryBlock[]): YearGroup[] => {
    const groups: YearGroup[] = [];
    for (const block of blocks) {
      const last = groups.at(-1);
      if (last?.year === block.year) {
        last.blocks.push(block);
      } else {
        groups.push({ year: block.year, blocks: [block] });
      }
    }
    return groups;
  };

  const groups = groupByYear(HISTORY);
</script>

<section class="history-page">
  <h1 style="font-size: 32px; margin: 0 0 24px 0; color: #07123b">ScottyLabs Over the Years</h1>
  <div class="history-box">
    <div
      style="display: flex; flex-direction: column; gap: 48px; font-family: Satoshi, sans-serif; color: #07123b"
    >
      {#each groups as group (group.year)}
        <div style="display: flex; flex-direction: column; gap: 16px">
          <div style="font-weight: 700; font-size: 20px; color: #07123b">
            {group.year}
          </div>
          {#each group.blocks as block (block.type === "carousel" ? `carousel-${block.photos[0].filename}` : `article-${block.url}`)}
            {#if block.type === "carousel"}
              <Carousel photos={block.photos} />
            {:else}
              <Article article={block} />
            {/if}
          {/each}
        </div>
      {/each}
    </div>
  </div>
</section>

<style>
  .history-page {
    padding: 48px 24px;
    max-width: 1400px;
    margin: 0 auto;
    @media (max-width: 768px) {
      padding: 32px 20px;
    }
  }
  .history-box {
    border-radius: 20px;
    border: 1px solid #b0b0b0;
    background: #f2f3f7;
    padding: 48px;
    @media (max-width: 768px) {
      padding: 24px 16px;
    }
  }
</style>
