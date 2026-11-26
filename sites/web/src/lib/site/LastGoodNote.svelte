<script lang="ts">
  // A calm note above public content shown from a saved copy (last-good.ts) because the site could
  // not load it just now, with a way to try again. Polite, so screen readers hear it once.
  import { lastGoodMessage } from "./last-good.ts";

  let {
    savedAt,
    onretry,
  }: {
    /** When the copy was loaded; null when it is what loaded earlier in this visit. */
    savedAt: number | null;
    onretry: () => void;
  } = $props();
</script>

<div class="last-good" role="status">
  <p class="last-good__text">{lastGoodMessage(savedAt)}</p>
  <button type="button" class="last-good__retry" onclick={onretry}>Try again</button>
</div>

<style>
  .last-good {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
    margin: 0 0 24px;
    padding: 12px 16px;
    border: 1px solid var(--p-border);
    border-radius: 12px;
    background: var(--p-surface-muted);
    color: var(--p-text-secondary);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .last-good__text {
    flex: 1 1 280px;
    margin: 0;
  }
  .last-good__retry {
    min-height: 36px;
    padding: 0 16px;
    border: 1px solid var(--p-border);
    border-radius: 100px;
    background: var(--p-surface);
    color: var(--p-text);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
  }
  .last-good__retry:hover {
    border-color: var(--p-text-secondary);
  }
  .last-good__retry:focus-visible {
    outline: 2px solid var(--p-focus);
    outline-offset: 2px;
  }
</style>
