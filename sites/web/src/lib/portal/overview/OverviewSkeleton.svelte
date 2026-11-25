<script lang="ts">
  // Placeholders the size of the overview while it loads, so nothing jumps when it arrives.
  import { Skeleton } from "../ui/index.ts";
</script>

<div class="overview-skeleton" aria-hidden="true">
  <div class="overview-skeleton__cards">
    {#each [0, 1, 2] as card (card)}
      <div class="overview-skeleton__card">
        <div class="overview-skeleton__row">
          <Skeleton width="32px" height="32px" radius="9px" />
          <Skeleton width="96px" height="12px" />
        </div>
        <Skeleton width="60%" height="20px" />
        <Skeleton width="85%" height="14px" />
        <Skeleton width="150px" height="32px" radius="999px" />
      </div>
    {/each}
  </div>
  <div class="overview-skeleton__lower">
    <div class="overview-skeleton__panel overview-skeleton__panel--wide">
      {#each [0, 1, 2] as line (line)}<Skeleton height="33px" />{/each}
    </div>
    <div class="overview-skeleton__panel">
      {#each [0, 1, 2] as line (line)}<Skeleton height="37px" />{/each}
    </div>
  </div>
</div>

<style>
  .overview-skeleton {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .overview-skeleton__cards,
  .overview-skeleton__lower {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 24px;
  }
  .overview-skeleton__card,
  .overview-skeleton__panel {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-height: 242px;
    padding: 20px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .overview-skeleton__row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .overview-skeleton__panel--wide {
    grid-column: span 2;
  }
  @media (max-width: 899px) {
    .overview-skeleton__cards,
    .overview-skeleton__lower {
      grid-template-columns: minmax(0, 1fr);
    }
    .overview-skeleton__card,
    .overview-skeleton__panel {
      min-height: 140px;
    }
    .overview-skeleton__panel--wide {
      grid-column: auto;
    }
  }
</style>
