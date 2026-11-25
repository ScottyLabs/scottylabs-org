<script lang="ts">
  // The committee tabs on the Projects page (Figma 50:8327): All and the seven committees with
  // how many projects each would show, the selector label over the current one and the gradient
  // underline. Each tab is a link to the filtered list; `onselect` lets the page filter in place.
  import type { CommitteeSlug } from "../api/types.ts";
  import { COMMITTEE_LIST } from "../portal/committees.ts";
  import type { CommitteeCounts } from "./filter.ts";

  let {
    selected,
    counts,
    href,
    onselect,
  }: {
    selected: CommitteeSlug | null;
    counts: CommitteeCounts | null;
    /** The address of the list filtered to a committee, or to All for null. */
    href: (committee: CommitteeSlug | null) => string;
    onselect: (committee: CommitteeSlug | null) => void;
  } = $props();

  const tabs = $derived([
    { slug: null, name: "All", color: null },
    ...COMMITTEE_LIST.map((committee) => ({
      slug: committee.slug as CommitteeSlug,
      name: committee.name,
      color: committee.color,
    })),
  ]);

  function select(event: MouseEvent, committee: CommitteeSlug | null): void {
    // Let people open a filtered list in a new tab or window.
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
    event.preventDefault();
    onselect(committee);
  }
</script>

{#snippet label(tab: { name: string; color: string | null }, count: number | null)}
  {#if tab.color !== null}
    <span class="filter__dot" style:background={tab.color} aria-hidden="true"></span>
  {/if}
  <span class="filter__name">{tab.name}</span>{#if count !== null}<span class="p-sr-only"
      >, {count} {count === 1 ? "project" : "projects"}</span
    ><span class="filter__count" aria-hidden="true">{count}</span>{/if}
{/snippet}

<nav class="filter" aria-label="Filter projects by committee">
  <ul class="filter__list" role="list">
    {#each tabs as tab (tab.slug ?? "all")}
      {@const current = tab.slug === selected}
      {@const count = counts === null ? null : counts[tab.slug ?? "all"]}
      <li class="filter__item">
        <span class="filter__selector" aria-hidden="true">[committee="{tab.slug ?? "*"}"]</span>
        {#if count === 0 && !current}
          <!-- Nothing to show there for this search, so it is not a choice. -->
          <span class="filter__tab filter__tab--empty" aria-disabled="true">
            {@render label(tab, count)}
          </span>
        {:else}
          <a
            class={["filter__tab", current && "filter__tab--current"]}
            href={href(tab.slug)}
            aria-current={current ? "true" : undefined}
            onclick={(event) => select(event, tab.slug)}
          >
            {@render label(tab, count)}
          </a>
        {/if}
      </li>
    {/each}
  </ul>
</nav>

<style>
  .filter {
    overflow-x: auto;
    scrollbar-width: none;
    font-family: var(--p-font);

    &::-webkit-scrollbar {
      display: none;
    }
  }
  .filter__list {
    /* Holds the visually hidden counts, so they scroll with the tabs instead of widening the page. */
    position: relative;
    display: grid;
    grid-template-columns: repeat(8, minmax(104px, 1fr));
    min-width: max-content;
    margin: 0;
    padding: 0;
  }
  .filter__item {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    padding-top: 16px;
  }
  /* Centered over the tab and allowed to be wider than it, as in the design. */
  .filter__selector {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    visibility: hidden;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .filter__item:has(.filter__tab--current) .filter__selector {
    visibility: visible;
  }
  .filter__tab {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px 10px 16px;
    border-bottom: 4px solid var(--p-border-muted);
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    white-space: nowrap;
    transition: color var(--p-duration) var(--p-ease);

    &:hover {
      color: var(--p-text);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -2px;
      border-radius: 6px 6px 0 0;
    }
  }
  .filter__tab--current {
    border-image: var(--p-tab-gradient) 1;
    color: var(--p-text);
  }
  .filter__tab--empty {
    color: var(--p-text-disabled);
    cursor: default;

    & .filter__count {
      color: var(--p-text-disabled);
    }
    & .filter__dot {
      opacity: 0.45;
    }
    &:hover {
      color: var(--p-text-disabled);
    }
  }
  .filter__dot {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
  }
  .filter__count {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
  }
</style>
