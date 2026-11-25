<script lang="ts">
  // The Ticker card: the six slots as a bar, what of the viewer's is live and what leaves next.
  import Megaphone from "@lucide/svelte/icons/megaphone";
  import type { OverviewResponse, Permissions } from "../../api/types.ts";
  import SectionCard from "./SectionCard.svelte";
  import { tickerCopy } from "./copy.ts";

  let {
    ticker,
    permissions,
    now,
  }: { ticker: OverviewResponse["ticker"]; permissions: Permissions; now: Date } = $props();

  const copy = $derived(tickerCopy(ticker, permissions, now));
  const slots = $derived(Array.from({ length: ticker.slots }, (_, index) => index < ticker.live));
</script>

<SectionCard icon={Megaphone} {copy} primaryAdds blocked={copy.blocked}>
  {#snippet visual()}
    <!-- The meta text above says the same ("5 of 6 slots"), so the bar is decorative. -->
    <span class="slots" aria-hidden="true">
      {#each slots as used, index (index)}<span class={["slot", used && "slot--used"]}></span>{/each}
    </span>
  {/snippet}
</SectionCard>

<style>
  .slots {
    display: flex;
    gap: 6px;
  }
  .slot {
    width: 40px;
    height: 6px;
    border-radius: 3px;
    background: var(--p-surface-strong);
  }
  .slot--used {
    background: var(--p-text);
  }
</style>
