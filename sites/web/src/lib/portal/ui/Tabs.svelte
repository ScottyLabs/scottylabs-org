<script lang="ts" module>
  export interface TabItem<T extends string = string> {
    value: T;
    label: string;
    count?: number;
    /** With an href on every tab, the tabs are links (for tabs kept in the URL). */
    href?: string;
    disabled?: boolean;
  }
</script>

<script lang="ts" generics="T extends string">
  import type { Snippet } from "svelte";

  let {
    tabs,
    value = $bindable(),
    label,
    onchange,
    children,
    class: className,
  }: {
    tabs: readonly TabItem<T>[];
    value?: T;
    /** Names the tab list for assistive technology. */
    label: string;
    onchange?: (value: T) => void;
    /** The panel for the selected tab. Only for button tabs; link tabs render their own page. */
    children?: Snippet<[T]>;
    class?: string;
  } = $props();

  const uid = $props.id();
  const asLinks = $derived(tabs.length > 0 && tabs.every((tab) => tab.href !== undefined));
  const selectedIndex = $derived(tabs.findIndex((tab) => tab.value === value));

  let list: HTMLElement | undefined = $state();
  const items: HTMLElement[] = $state([]);
  let indicator = $state<{ left: number; width: number } | null>(null);

  const tabId = (tab: TabItem<T>) => `tab-${uid}-${tab.value}`;
  const panelId = `tabpanel-${uid}`;

  function measure() {
    const element = items[selectedIndex];
    if (list === undefined || element === undefined) {
      indicator = null;
      return;
    }
    indicator = { left: element.offsetLeft, width: element.offsetWidth };
  }

  $effect(() => {
    void selectedIndex;
    void tabs.length;
    measure();
    if (list === undefined) return;
    const observer = new ResizeObserver(measure);
    observer.observe(list);
    return () => observer.disconnect();
  });

  function select(index: number) {
    const tab = tabs[index];
    if (tab === undefined || tab.disabled || tab.value === value) return;
    value = tab.value;
    onchange?.(tab.value);
  }

  function step(from: number, direction: 1 | -1): number {
    for (let i = 1; i <= tabs.length; i++) {
      const index = (from + direction * i + tabs.length) % tabs.length;
      if (!tabs[index]?.disabled) return index;
    }
    return from;
  }

  function onkeydown(event: KeyboardEvent, index: number) {
    let next: number | null = null;
    if (event.key === "ArrowRight") next = step(index, 1);
    else if (event.key === "ArrowLeft") next = step(index, -1);
    else if (event.key === "Home") next = step(-1, 1);
    else if (event.key === "End") next = step(tabs.length, -1);
    if (next === null) return;
    event.preventDefault();
    select(next);
    items[next]?.focus();
  }
</script>

{#snippet tabContent(tab: TabItem)}
  <span>{tab.label}</span>
  {#if tab.count !== undefined}<span class="p-tabs__count">{tab.count}</span>{/if}
{/snippet}

<div
  class={["p-tabs", className]}
  style:--indicator-left={indicator ? `${indicator.left}px` : undefined}
  style:--indicator-width={indicator ? `${indicator.width}px` : undefined}
>
  {#if asLinks}
    <nav aria-label={label}>
      <ul class="p-tabs__list" bind:this={list}>
        {#each tabs as tab, index (tab.value)}
          <li class="p-tabs__item">
            <a
              bind:this={items[index]}
              class="p-tabs__tab"
              href={tab.disabled ? undefined : tab.href}
              aria-current={index === selectedIndex ? "page" : undefined}
              aria-disabled={tab.disabled ? "true" : undefined}
              onclick={() => select(index)}
            >
              {@render tabContent(tab)}
            </a>
          </li>
        {/each}
      </ul>
    </nav>
  {:else}
    <div class="p-tabs__list" role="tablist" aria-label={label} bind:this={list}>
      {#each tabs as tab, index (tab.value)}
        <button
          bind:this={items[index]}
          type="button"
          role="tab"
          id={tabId(tab)}
          class="p-tabs__tab"
          aria-selected={index === selectedIndex ? "true" : "false"}
          aria-controls={panelId}
          tabindex={index === selectedIndex || (selectedIndex < 0 && index === 0) ? 0 : -1}
          disabled={tab.disabled}
          onclick={() => select(index)}
          onkeydown={(event) => onkeydown(event, index)}
        >
          {@render tabContent(tab)}
        </button>
      {/each}
    </div>
  {/if}
  <span class={["p-tabs__indicator", indicator && "p-tabs__indicator--ready"]} aria-hidden="true"></span>
</div>

{#if !asLinks && children && value !== undefined}
  {@const selected = tabs[selectedIndex]}
  <div
    class="p-tabs__panel"
    role="tabpanel"
    id={panelId}
    aria-labelledby={selected ? tabId(selected) : undefined}
    tabindex="0"
  >
    {@render children(value)}
  </div>
{/if}

<style>
  /* The wrapper is the scroller and holds the indicator, so the indicator moves with the tabs
     when they scroll sideways on narrow screens. */
  .p-tabs {
    --track: 3px;
    position: relative;
    padding-bottom: var(--track);
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scrollbar-width: none;
    font-family: var(--p-font);
    /* The grey track stays put under the visible tabs while the indicator scrolls with them. */
    background: linear-gradient(var(--p-border-muted), var(--p-border-muted)) left bottom / 100%
      var(--track) no-repeat;

    &::-webkit-scrollbar {
      display: none;
    }
  }
  .p-tabs__list {
    display: flex;
    gap: 24px;
    width: max-content;
    min-width: 100%;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .p-tabs__item {
    display: flex;
  }
  .p-tabs__tab {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 2px 11px;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
    white-space: nowrap;
    text-decoration: none;
    cursor: pointer;
    border-radius: 6px 6px 0 0;
    transition: color var(--p-duration) var(--p-ease);

    &:hover {
      color: var(--p-text);
    }
    &[aria-selected="true"],
    &[aria-current="page"] {
      color: var(--p-text);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -2px;
    }
    &:disabled,
    &[aria-disabled="true"] {
      color: var(--p-text-disabled);
      cursor: not-allowed;
    }
  }
  .p-tabs__count {
    min-width: 20px;
    padding: 0 6px;
    border-radius: var(--p-radius-pill);
    background: var(--p-surface-strong);
    color: var(--p-text);
    font-size: 0.75rem;
    line-height: 18px;
    text-align: center;
    font-variant-numeric: tabular-nums;
    box-sizing: border-box;
  }
  .p-tabs__indicator {
    position: absolute;
    left: 0;
    bottom: 0;
    width: var(--indicator-width, 0);
    height: var(--track);
    border-radius: var(--track);
    background: var(--p-tab-gradient);
    opacity: 0;
    transform: translateX(var(--indicator-left, 0));
  }
  .p-tabs__indicator--ready {
    opacity: 1;
    transition:
      transform 0.45s cubic-bezier(0.27, 1.32, 0.41, 1),
      width 0.45s cubic-bezier(0.27, 1.32, 0.41, 1);
  }
  .p-tabs__panel:focus-visible {
    outline: 2px solid var(--p-focus);
    outline-offset: 4px;
    border-radius: 4px;
  }
  @media (prefers-reduced-motion: reduce) {
    .p-tabs__indicator--ready {
      transition: none;
    }
  }
</style>
