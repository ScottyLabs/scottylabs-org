<script lang="ts">
  // The ticker as the home page shows it right now, with the slots in use.
  import type { Announcement } from "../../api/types.ts";
  import StatusDot from "../../portal/ui/StatusDot.svelte";
  import Ticker from "../Ticker.svelte";

  let {
    announcements,
    used,
    total,
  }: { announcements: readonly Announcement[] | null; used: number | null; total: number } = $props();
</script>

<section class="live-preview" aria-labelledby="live-preview-heading">
  <header class="live-preview__head">
    <h2 id="live-preview-heading" class="live-preview__title">
      <StatusDot tone="success" size="sm" />
      Live now on scottylabs.org
    </h2>
    {#if used !== null}<span class="live-preview__slots">{used} of {total} slots used</span>{/if}
  </header>
  {#if announcements === null}
    <div class="live-preview__placeholder" aria-hidden="true"></div>
  {:else if announcements.length === 0}
    <p class="live-preview__empty">The ticker is hidden on the home page until something is live.</p>
  {:else}
    <Ticker {announcements} variant="card" postHref={null} />
  {/if}
</section>

<style>
  .live-preview {
    overflow: clip;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .live-preview__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 44px;
    padding: 0 20px;
    border-bottom: 1px solid var(--p-border);
  }
  .live-preview__title {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .live-preview__slots {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .live-preview__placeholder {
    height: 68px;
  }
  .live-preview__empty {
    margin: 0;
    padding: 24px 20px;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
</style>
