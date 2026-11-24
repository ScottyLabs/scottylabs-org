<script lang="ts">
  // The home page ticker (spec T-1 to T-12). Live announcements scroll right to left at 40 px a
  // second with the dog mark between them, pause on hover, keyboard focus or the pause button,
  // and open as a list from "All N". With reduced motion it shows one at a time with previous and
  // next, and the position opens the list. One announcement is shown still when it fits and
  // scrolls when it does not; none, or an unreachable API, renders nothing.
  //
  // Screen readers get the real announcements as a plain list. The copies that make the loop
  // seamless are hidden from them and skipped by the keyboard.
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import X from "@lucide/svelte/icons/x";
  import { onMount } from "svelte";
  import { get, isAbortError } from "../api/client.ts";
  import type { PublicAnnouncement, PublicTickerResponse } from "../api/types.ts";
  import DogMark from "../portal/ui/DogMark.svelte";
  import TickerItem from "./TickerItem.svelte";
  import TickerList from "./TickerList.svelte";
  import { offersList, reservesSpace, tickerMode } from "./mode.ts";
  import { Marquee, readLastCount, readPaused, writeLastCount, writePaused } from "./motion.ts";

  type Item = Pick<PublicAnnouncement, "id" | "kind" | "committee" | "message" | "linkUrl">;

  let {
    announcements: provided,
    variant = "band",
    postHref = "/portal/ticker/new/",
    class: className,
  }: {
    /** Shown instead of the public feed, for the portal's preview of what is live. */
    announcements?: readonly Item[];
    /** "band" spans the page under the hero; "card" sits inside a portal card. */
    variant?: "band" | "card";
    /** The "Post an announcement" link under the list, or null to leave it out. */
    postHref?: string | null;
    class?: string;
  } = $props();

  /** Visitors who keep the page open still lose expired announcements. */
  const REFRESH_MS = 5 * 60_000;

  const uid = $props.id();
  const listId = `ticker-list-${uid}`;

  let fetched = $state.raw<readonly Item[] | null>(null);
  let failed = $state(false);
  const items = $derived<readonly Item[] | null>(provided ?? (failed ? [] : fetched));
  /** Hold the strip's height while loading, unless the feed was empty last time. */
  let reserve = $state(true);

  let reducedMotion = $state(false);
  let userPaused = $state(false);
  let hovering = $state(false);
  let focused = $state(false);
  let listOpen = $state(false);
  let step = $state(0);
  let stepNotice = $state("");
  /** The only announcement does not fit standing still, so it scrolls. */
  let tooWide = $state(false);
  /** The announcement standing still is cut short. */
  let cutShort = $state(false);

  const mode = $derived(tickerMode({ count: items?.length ?? null, reducedMotion, tooWide }));
  const listed = $derived(offersList(mode, cutShort));
  const current = $derived(items === null ? 0 : Math.min(step, Math.max(items.length - 1, 0)));

  onMount(() => {
    userPaused = readPaused();
    if (provided === undefined) reserve = reservesSpace(readLastCount());
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = query.matches;
    const update = () => (reducedMotion = query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  $effect(() => {
    if (provided !== undefined) return;
    const controller = new AbortController();
    void load(controller.signal);
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") void load(controller.signal);
    }, REFRESH_MS);
    return () => {
      controller.abort();
      clearInterval(timer);
    };
  });

  async function load(signal: AbortSignal) {
    try {
      const response = await get<PublicTickerResponse>("/public/ticker", {
        signal,
        redirectOnUnauthenticated: false,
      });
      if (!sameItems(fetched, response.announcements)) fetched = response.announcements;
      failed = false;
      writeLastCount(response.announcements.length);
    } catch (error) {
      if (isAbortError(error)) return;
      // Keep showing what loaded before; with nothing loaded the strip stays hidden.
      if (fetched === null) failed = true;
    }
  }

  function sameItems(before: readonly Item[] | null, after: readonly Item[]): boolean {
    return (
      before !== null &&
      before.length === after.length &&
      before.every((item, index) => JSON.stringify(item) === JSON.stringify(after[index]))
    );
  }

  // The moving track.
  let windowEl = $state<HTMLElement>();
  let trackEl = $state<HTMLElement>();
  let setEl = $state<HTMLElement>();
  let capEl = $state<HTMLElement>();
  let allButton = $state<HTMLElement>();
  let controlsEl = $state<HTMLElement>();
  let controlsWidth = $state(0);
  let marquee = $state.raw<Marquee | null>(null);
  let setWidth = $state(0);
  let windowWidth = $state(0);

  /** Copies after the real set: enough to fill the window at the far end of the loop. */
  const copiesAfter = $derived(setWidth > 0 ? 1 + Math.ceil(windowWidth / setWidth) : 1);
  const moving = $derived(!userPaused && !hovering && !focused && !listOpen);

  $effect(() => {
    if (mode !== "marquee" || trackEl === undefined) return;
    const created = new Marquee(trackEl);
    marquee = created;
    return () => {
      created.destroy();
      marquee = null;
    };
  });

  $effect(() => {
    const set = setEl;
    const view = windowEl;
    if (mode !== "marquee" || set === undefined || view === undefined) return;
    const observer = new ResizeObserver(() => {
      setWidth = set.getBoundingClientRect().width;
      windowWidth = view.getBoundingClientRect().width;
    });
    observer.observe(set);
    observer.observe(view);
    return () => observer.disconnect();
  });

  $effect(() => {
    if (marquee !== null && setWidth > 0) marquee.measure(setWidth);
  });

  $effect(() => {
    if (marquee === null) return;
    if (moving) marquee.play();
    else marquee.pause();
  });

  // A different announcement may fit standing still, so look again.
  const itemsKey = $derived(items?.map((item) => `${item.id}\n${item.message}\n${item.committee}`).join("\n") ?? "");
  $effect(() => {
    void itemsKey;
    tooWide = false;
  });

  /** The window's width when the only announcement was found too wide for it. */
  let tooWideAt = 0;

  /** Checks whether the announcement standing still is cut short; if it may scroll, it then does. */
  function measureStill() {
    const view = windowEl;
    const message = view?.querySelector<HTMLElement>(".ticker__step--current .ticker-item__message");
    if (view === undefined || message === null || message === undefined) return;
    cutShort = message.scrollWidth > message.clientWidth + 1 || message.scrollHeight > message.clientHeight + 1;
    if (cutShort && !reducedMotion) {
      tooWideAt = view.clientWidth;
      tooWide = true;
    }
  }

  $effect(() => {
    const view = windowEl;
    if (mode !== "static" || view === undefined) return;
    void itemsKey;
    let active = true;
    measureStill();
    const observer = new ResizeObserver(() => measureStill());
    observer.observe(view);
    // Widths change once the web font arrives.
    void document.fonts.ready.then(() => {
      if (active) measureStill();
    });
    return () => {
      active = false;
      observer.disconnect();
    };
  });

  // Scrolling because it did not fit: once the strip is wider, try standing still again.
  $effect(() => {
    if (mode === "marquee" && tooWide && windowWidth > tooWideAt + 1) tooWide = false;
  });

  /** Brings a focused item into view just right of the left fade. */
  function reveal(event: FocusEvent) {
    focused = true;
    const entry = event.currentTarget instanceof HTMLElement ? event.currentTarget.closest("li") : null;
    if (marquee === null || entry === null) return;
    marquee.reveal(entry.offsetLeft, capEl?.offsetWidth ?? 0);
  }

  function onFocusIn() {
    const view = windowEl;
    if (view === undefined) return;
    unscroll(view);
    requestAnimationFrame(() => unscroll(view));
  }

  /**
   * Focusing an item that is off to the side makes the browser scroll every clipping ancestor
   * toward it, such as a card with hidden overflow. The ticker shows the item itself, so put them back.
   */
  function unscroll(node: HTMLElement) {
    for (let element = node.parentElement; element !== null; element = element.parentElement) {
      if (element.scrollLeft !== 0 && getComputedStyle(element).overflowX === "hidden") element.scrollLeft = 0;
    }
  }

  function onFocusOut(event: FocusEvent) {
    if (!(event.relatedTarget instanceof Node) || !windowEl?.contains(event.relatedTarget)) focused = false;
  }

  function togglePause() {
    userPaused = !userPaused;
    writePaused(userPaused);
  }

  function stepBy(delta: number) {
    if (items === null || items.length === 0) return;
    step = (current + delta + items.length) % items.length;
    stepNotice = `${step + 1} of ${items.length}: ${items[step]?.message ?? ""}`;
  }
</script>

{#snippet entry(item: Item, focusable: boolean)}
  <TickerItem announcement={item} {focusable} onfocus={focusable ? reveal : undefined} />
  <DogMark variant="gradient" width={29} />
{/snippet}

{#snippet listButton(label: string, name: string)}
  <button
    type="button"
    class="ticker__button ticker__button--all"
    popovertarget={listId}
    aria-expanded={listOpen ? "true" : "false"}
    aria-controls={listId}
    bind:this={allButton}
  >
    {label}<span class="p-sr-only">{name}</span>
    {#if listOpen}<X size={14} strokeWidth={2} aria-hidden="true" />{:else}<ChevronDown size={14} strokeWidth={2} aria-hidden="true" />{/if}
  </button>
{/snippet}

{#if mode === "loading"}
  {#if reserve}
    <!-- Holds the strip's height until the announcements arrive, so nothing below moves. -->
    <div class={["ticker", `ticker--${variant}`, "ticker--loading", className]} aria-hidden="true"></div>
  {/if}
{:else if items !== null && items.length > 0}
  <section
    class={["ticker", `ticker--${variant}`, `ticker--${mode}`, reducedMotion && "ticker--reduced", className]}
    style:--controls="{mode !== "marquee" && listed ? controlsWidth : 0}px"
    aria-label="Announcements"
    onpointerenter={() => (hovering = true)}
    onpointerleave={() => (hovering = false)}
  >
    <div class="ticker__window" bind:this={windowEl} onfocusin={onFocusIn} onfocusout={onFocusOut}>
      {#if mode === "marquee"}
        <div class="ticker__track" bind:this={trackEl}>
          <div class="ticker__set" aria-hidden="true">
            {#each items as item (item.id)}<div class="ticker__entry">{@render entry(item, false)}</div>{/each}
          </div>
          <ul class="ticker__set" bind:this={setEl}>
            {#each items as item (item.id)}<li class="ticker__entry">{@render entry(item, true)}</li>{/each}
          </ul>
          {#each { length: copiesAfter }, copy (copy)}
            <div class="ticker__set" aria-hidden="true">
              {#each items as item (item.id)}<div class="ticker__entry">{@render entry(item, false)}</div>{/each}
            </div>
          {/each}
        </div>
      {:else if mode === "stepper"}
        <ul class="ticker__steps">
          {#each items as item, index (item.id)}
            <li class={["ticker__step", index === current && "ticker__step--current"]} onfocusin={() => (step = index)}>
              <TickerItem announcement={item} />
            </li>
          {/each}
        </ul>
      {:else}
        <ul class="ticker__steps">
          <li class="ticker__step ticker__step--current"><TickerItem announcement={items[0]} /></li>
        </ul>
      {/if}
    </div>

    <div class="ticker__cap ticker__cap--left" bind:this={capEl} aria-hidden="true"></div>
    <div class="ticker__cap ticker__cap--right" aria-hidden="true"></div>

    <div class="ticker__overlay">
      <span class="ticker__label" aria-hidden="true">[aria-live="polite"]</span>
      {#if mode === "marquee"}
        <div class="ticker__controls" bind:this={controlsEl}>
          {@render listButton(`All ${items.length}`, items.length === 1 ? " announcement" : " announcements")}
          <button
            type="button"
            class="ticker__button ticker__button--icon"
            aria-label={userPaused ? "Play announcements" : "Pause announcements"}
            title={userPaused ? "Play" : "Pause"}
            onclick={togglePause}
          >
            {#if userPaused}<Play size={14} strokeWidth={2} aria-hidden="true" />{:else}<Pause size={14} strokeWidth={2} aria-hidden="true" />{/if}
          </button>
        </div>
      {:else if mode === "stepper"}
        <div class="ticker__controls" bind:this={controlsEl} bind:offsetWidth={controlsWidth}>
          {@render listButton(`${current + 1} of ${items.length}`, `, show all ${items.length} announcements`)}
          <button type="button" class="ticker__button ticker__button--icon" aria-label="Previous announcement" title="Previous" onclick={() => stepBy(-1)}>
            <ChevronLeft size={14} strokeWidth={2} aria-hidden="true" />
          </button>
          <button type="button" class="ticker__button ticker__button--icon" aria-label="Next announcement" title="Next" onclick={() => stepBy(1)}>
            <ChevronRight size={14} strokeWidth={2} aria-hidden="true" />
          </button>
          <span class="p-sr-only" aria-live="polite">{stepNotice}</span>
        </div>
      {:else if listed}
        <div class="ticker__controls" bind:this={controlsEl} bind:offsetWidth={controlsWidth}>
          {@render listButton(`All ${items.length}`, items.length === 1 ? " announcement" : " announcements")}
        </div>
      {/if}
    </div>

    {#if listed}
      <TickerList id={listId} {items} anchor={controlsEl} invoker={allButton} {postHref} bind:open={listOpen} />
    {/if}
  </section>
{/if}

<style>
  .ticker {
    /* Where the label and controls line up: the content edge of .centered-section on the home page. */
    --edge: max(40px, calc((100% - 1200px) / 2));
    --left-cap: calc(var(--edge) + 214px);
    --right-cap: calc(var(--edge) + 176px);
    --still-left: calc(var(--edge) + 240px);
    --fade-color: 255, 255, 255;
    --fade: 56px;
    position: relative;
    box-sizing: border-box;
    height: 68px;
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }
  .ticker--band {
    border-block: 1px solid var(--p-border);
    background: rgba(255, 255, 255, 0.94);
  }
  .ticker--card {
    --edge: 20px;
    background: var(--p-surface);
  }

  .ticker__window {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    /* Clip without being a scroll container, so focusing an item cannot scroll the window. */
    overflow: hidden;
    overflow: clip;
  }
  /* Standing still, the announcement takes the room between the label and the controls. */
  .ticker--static .ticker__window,
  .ticker--stepper .ticker__window {
    padding-inline: var(--still-left) calc(var(--edge) + var(--controls, 0px));
  }

  .ticker__track {
    /* The items' offsetLeft is measured from here. */
    position: relative;
    display: flex;
    align-items: center;
    width: max-content;
    will-change: transform;
  }
  .ticker__set {
    display: flex;
    align-items: center;
    flex: none;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .ticker__entry {
    display: flex;
    align-items: center;
    flex: none;
    gap: 28px;
    padding-right: 28px;
  }

  .ticker__steps {
    display: flex;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* Steps other than the current one stay in the list for screen readers and the keyboard. */
  .ticker__step:not(.ticker__step--current) {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .ticker__step--current {
    display: flex;
    min-width: 0;

    & :global(.ticker-item) {
      flex: 0 1 auto;
      min-width: 0;
      max-width: 100%;
    }
  }

  .ticker__cap {
    position: absolute;
    top: 0;
    bottom: 0;
    pointer-events: none;
  }
  .ticker__cap--left {
    left: 0;
    width: var(--left-cap);
    background: linear-gradient(90deg, rgb(var(--fade-color)) calc(100% - var(--fade)), rgba(var(--fade-color), 0) 100%);
  }
  .ticker__cap--right {
    right: 0;
    width: var(--right-cap);
    background: linear-gradient(90deg, rgba(var(--fade-color), 0) 0, rgb(var(--fade-color)) var(--fade));
  }
  .ticker--static .ticker__cap,
  .ticker--stepper .ticker__cap {
    display: none;
  }

  .ticker__overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding-inline: var(--edge);
    pointer-events: none;
  }
  .ticker__label {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .ticker__controls {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    /* Keeps a gap between a still announcement and the controls. */
    padding-left: 16px;
    pointer-events: auto;
  }
  .ticker__button {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    height: 30px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-sm);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    white-space: nowrap;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .ticker__button--all {
    padding: 6px 8px 6px 12px;
    font-variant-numeric: tabular-nums;
  }
  .ticker__button--icon {
    width: 30px;
    padding: 0;
  }

  @media (max-width: 768px) {
    .ticker--band {
      --edge: 20px;
    }
  }
  /* Phones keep the pause button and drop the label. The scrolling text reads in full there, so
     the list is offered only where an announcement stands still. */
  @media (max-width: 600px) {
    .ticker {
      --edge: 20px;
      --still-left: var(--edge);
      --left-cap: 32px;
      --right-cap: 80px;
      --fade: 28px;
    }
    .ticker__label,
    .ticker--marquee .ticker__button--all {
      display: none;
    }
    /* Standing still on a phone, the controls leave little room: the message takes two lines and
       the committee is left to the list (screen readers still hear it). */
    .ticker--reduced .ticker__step--current {
      & :global(.ticker-item) {
        height: auto;
        min-height: 36px;
        padding-block: 6px;
      }
      & :global(.ticker-item__message) {
        display: -webkit-box;
        -webkit-box-orient: vertical;
        -webkit-line-clamp: 2;
        line-clamp: 2;
        font-size: 0.875rem;
        line-height: 18px;
        white-space: normal;
        overflow-wrap: anywhere;
      }
      & :global(.p-chip) {
        position: absolute;
        width: 1px;
        height: 1px;
        overflow: hidden;
        clip-path: inset(50%);
        white-space: nowrap;
      }
    }
  }
</style>
