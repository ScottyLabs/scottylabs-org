<script lang="ts">
  // Every live announcement in a popover under the "All N" button. It sits in the top layer, so
  // the hero's clipping cannot cut it off; Escape and clicks outside close it, and focus goes
  // back to the button.
  import type { PublicAnnouncement } from "../api/types.ts";
  import CommitteeChip from "../portal/ui/CommitteeChip.svelte";
  import { linkHost } from "./format.ts";
  import { kindIcon } from "./kinds.ts";

  type Item = Pick<PublicAnnouncement, "id" | "kind" | "committee" | "message" | "linkUrl">;

  let {
    id,
    items,
    anchor,
    invoker,
    postHref,
    open = $bindable(false),
  }: {
    id: string;
    items: readonly Item[];
    /** The list opens under this element, lined up with its right edge. */
    anchor: HTMLElement | undefined;
    /** The button that opens the list, which gets focus back when it closes. */
    invoker: HTMLElement | undefined;
    /** Where members can post one, or null to leave the line out. */
    postHref: string | null;
    open?: boolean;
  } = $props();

  const headingId = $derived(`${id}-heading`);
  let panel = $state<HTMLElement>();
  let heading = $state<HTMLElement>();

  function place() {
    if (panel === undefined || anchor === undefined) return;
    const rect = anchor.getBoundingClientRect();
    const top = Math.round(rect.bottom + 8);
    panel.style.setProperty("--list-top", `${top}px`);
    panel.style.setProperty("--list-right", `${Math.max(16, Math.round(window.innerWidth - rect.right))}px`);
  }

  function ontoggle(event: Event) {
    const opened = event instanceof ToggleEvent ? event.newState === "open" : panel?.matches(":popover-open") === true;
    open = opened;
    if (opened) {
      place();
      heading?.focus();
      return;
    }
    // Light dismiss by clicking elsewhere leaves focus where the click went; Escape returns it here.
    if (panel?.contains(document.activeElement) === true || document.activeElement === document.body) {
      invoker?.focus();
    }
  }

  $effect(() => {
    if (!open) return;
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, { passive: true, capture: true });
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, { capture: true });
    };
  });
</script>

<div {id} class="ticker-list" popover="auto" role="dialog" aria-labelledby={headingId} bind:this={panel} {ontoggle}>
  <div class="ticker-list__heading">
    <h2 id={headingId} class="ticker-list__title" tabindex="-1" bind:this={heading}>Announcements</h2>
    <span class="ticker-list__count">{items.length} live now</span>
  </div>
  <ul class="ticker-list__items">
    {#each items as item (item.id)}
      {@const Icon = kindIcon(item.kind)}
      {@const host = linkHost(item.linkUrl)}
      <li>
        {#snippet row()}
          <span class="ticker-list__icon"><Icon size={16} strokeWidth={1.75} aria-hidden="true" /></span>
          <span class="ticker-list__text">
            <span class="ticker-list__message">{item.message}</span>
            <CommitteeChip committee={item.committee} />
          </span>
        {/snippet}
        {#if host !== null && item.linkUrl}
          <a class="ticker-list__row ticker-list__row--link" href={item.linkUrl} target="_blank" rel="noopener noreferrer">
            {@render row()}
            <span class="p-sr-only">(opens {host} in a new tab)</span>
          </a>
        {:else}
          <div class="ticker-list__row">{@render row()}</div>
        {/if}
      </li>
    {/each}
  </ul>
  {#if postHref !== null}
    <div class="ticker-list__footer">
      <a class="ticker-list__post" href={postHref}>ScottyLabs member? Post an announcement</a>
    </div>
  {/if}
</div>

<style>
  .ticker-list {
    position: fixed;
    inset: auto;
    top: var(--list-top, 0);
    right: var(--list-right, 16px);
    box-sizing: border-box;
    width: min(440px, calc(100vw - 32px));
    max-height: calc(100dvh - var(--list-top, 0px) - 16px);
    margin: 0;
    padding: 18px 10px 12px;
    overflow-y: auto;
    overscroll-behavior: contain;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-pop);
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;

    &:popover-open {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
  }
  .ticker-list__heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding: 0 10px 8px;
  }
  .ticker-list__title {
    margin: 0;
    color: var(--p-text);
    font-size: 1rem;
    font-weight: 500;
    line-height: 24px;

    &:focus {
      outline: none;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
      border-radius: 4px;
    }
  }
  .ticker-list__count {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .ticker-list__items {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .ticker-list__row {
    display: flex;
    gap: 12px;
    padding: 10px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text);
    text-decoration: none;
  }
  .ticker-list__row--link {
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -2px;
      background: var(--p-surface-muted);
    }
  }
  .ticker-list__icon {
    display: flex;
    flex: none;
    padding-top: 2px;
    color: var(--p-text);
  }
  .ticker-list__text {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    min-width: 0;
  }
  .ticker-list__message {
    font-size: 0.875rem;
    line-height: 20px;
    overflow-wrap: anywhere;
  }
  .ticker-list__footer {
    margin-top: 4px;
    padding: 10px 10px 4px;
    border-top: 1px solid var(--p-border);
  }
  .ticker-list__post {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
    text-decoration: underline;
    text-underline-offset: 2px;

    &:hover {
      color: var(--p-text);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
      border-radius: 2px;
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .ticker-list:popover-open {
      animation: ticker-list-in var(--p-duration) var(--p-ease);
    }
  }
  @keyframes ticker-list-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
