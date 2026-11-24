<script lang="ts">
  // One announcement as the ticker shows it: type icon, message, committee chip and, for links,
  // an arrow. The whole item is the link. Hover and keyboard focus show the selection box with
  // the link's host, as in the design's inspector style.
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import type { PublicAnnouncement } from "../api/types.ts";
  import CommitteeChip from "../portal/ui/CommitteeChip.svelte";
  import { linkHost } from "./format.ts";
  import { kindIcon } from "./kinds.ts";

  let {
    announcement,
    focusable = true,
    onfocus,
    class: className,
  }: {
    announcement: Pick<PublicAnnouncement, "kind" | "committee" | "message" | "linkUrl">;
    /** False for the moving copies, which mouse users can click but the keyboard skips. */
    focusable?: boolean;
    onfocus?: (event: FocusEvent) => void;
    class?: string;
  } = $props();

  const Icon = $derived(kindIcon(announcement.kind));
  const host = $derived(linkHost(announcement.linkUrl));
</script>

{#snippet body()}
  <Icon class="ticker-item__icon" size={16} strokeWidth={1.75} aria-hidden="true" />
  <span class="ticker-item__message">{announcement.message}</span>
  <CommitteeChip committee={announcement.committee} />
{/snippet}

{#if host !== null && announcement.linkUrl}
  <a
    class={["ticker-item", "ticker-item--link", className]}
    href={announcement.linkUrl}
    target="_blank"
    rel="noopener noreferrer"
    tabindex={focusable ? undefined : -1}
    {onfocus}
  >
    {@render body()}
    <ArrowUpRight class="ticker-item__arrow" size={14} strokeWidth={2} aria-hidden="true" />
    <span class="p-sr-only">(opens {host} in a new tab)</span>
    <span class="ticker-item__host" aria-hidden="true">{host}</span>
    <span class="ticker-item__handle ticker-item__handle--tl" aria-hidden="true"></span>
    <span class="ticker-item__handle ticker-item__handle--tr" aria-hidden="true"></span>
    <span class="ticker-item__handle ticker-item__handle--bl" aria-hidden="true"></span>
    <span class="ticker-item__handle ticker-item__handle--br" aria-hidden="true"></span>
  </a>
{:else}
  <span class={["ticker-item", className]}>{@render body()}</span>
{/if}

<style>
  .ticker-item {
    --selection: var(--p-selection);
    position: relative;
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    flex: none;
    gap: 10px;
    height: 36px;
    padding: 7px 12px;
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.9375rem;
    font-weight: 500;
    line-height: 22px;
    white-space: nowrap;
    text-decoration: none;
    outline: none;

    & :global(.ticker-item__icon),
    & :global(.ticker-item__arrow) {
      flex: none;
      color: var(--p-text);
    }
  }

  /* The selection box: a 2px frame, square handles at the corners and the host above. */
  .ticker-item--link::before {
    content: "";
    position: absolute;
    inset: 0;
    border: 2px solid var(--selection);
    opacity: 0;
    pointer-events: none;
  }
  .ticker-item__host,
  .ticker-item__handle {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .ticker-item__host {
    left: 0;
    bottom: 100%;
    padding: 2px 6px;
    /* Darker than the frame so the white text meets 4.5:1. */
    background: var(--p-info-text);
    color: #ffffff;
    font-family: var(--p-mono);
    font-size: 0.625rem;
    font-weight: 400;
    line-height: 12px;
  }
  .ticker-item__handle {
    width: 7px;
    height: 7px;
    box-sizing: border-box;
    border: 1.5px solid var(--selection);
    background: #ffffff;
  }
  .ticker-item__handle--tl {
    top: -3px;
    left: -3px;
  }
  .ticker-item__handle--tr {
    top: -3px;
    right: -3px;
  }
  .ticker-item__handle--bl {
    bottom: -3px;
    left: -3px;
  }
  .ticker-item__handle--br {
    bottom: -3px;
    right: -3px;
  }
  .ticker-item--link:hover,
  .ticker-item--link:focus-visible {
    &::before,
    & .ticker-item__host,
    & .ticker-item__handle {
      opacity: 1;
    }
  }
  /* The keyboard frame uses the focus blue, which keeps 3:1 against white. */
  .ticker-item--link:focus-visible {
    --selection: var(--p-focus);
  }
  /* Cut short with an ellipsis when the strip makes the item narrower than its text. */
  .ticker-item__message {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
