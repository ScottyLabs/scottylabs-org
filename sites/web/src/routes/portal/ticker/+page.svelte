<script lang="ts">
  // The ticker in the portal (spec TK-1 to TK-9): what is live now, the slots in use, and the
  // live, waiting and expired announcements, with extend, end, reorder and post again.
  import Archive from "@lucide/svelte/icons/archive";
  import Hourglass from "@lucide/svelte/icons/hourglass";
  import Lock from "@lucide/svelte/icons/lock";
  import Megaphone from "@lucide/svelte/icons/megaphone";
  import Plus from "@lucide/svelte/icons/plus";
  import { tick } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { page } from "$app/state";
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { Announcement, AnnouncementListResponse, AnnouncementTab } from "#lib/api/types.ts";
  import { formatDateTimeLong, getPortalSession, getToasts } from "#lib/portal/index.ts";
  import { Button, Callout, Dialog, EmptyState, PageHeader, Skeleton, Tabs, type TabItem } from "#lib/portal/ui/index.ts";
  import { focusPageHeading } from "#lib/portal/ui/focus.ts";
  import AnnouncementList from "#lib/ticker/portal/AnnouncementList.svelte";
  import LivePreview from "#lib/ticker/portal/LivePreview.svelte";
  import {
    canReorder,
    deleteAnnouncement,
    endAnnouncement,
    extendAnnouncement,
    listAnnouncements,
    postsDirectly,
    reorderAnnouncements,
  } from "#lib/ticker/portal/api.ts";
  import { addedMessage, appendPage } from "#lib/ticker/portal/paging.ts";
  import { MAX_LIVE, quoted } from "#lib/ticker/format.ts";

  const session = getPortalSession();
  const toasts = getToasts();
  const permissions = $derived(session.permissions);
  // Phones get the compact layout of the mobile design: the live strip under the top bar, then a short heading.
  const phone = new MediaQuery("(max-width: 600px)");

  const BASE = "/portal/ticker/";

  function parseTab(value: string | null): AnnouncementTab {
    return value === "waiting" || value === "expired" ? value : "live";
  }

  const tab = $derived(parseTab(page.url.searchParams.get("tab")));
  const highlight = $derived(page.url.searchParams.get("highlight"));

  let live = $state.raw<AnnouncementListResponse | null>(null);
  let shown = $state.raw<{ tab: AnnouncementTab; data: AnnouncementListResponse } | null>(null);
  let loadError = $state<string | null>(null);
  let now = $state(new Date());
  let reloads = $state(0);

  $effect(() => {
    const which = tab;
    void reloads;
    const controller = new AbortController();
    void load(which, controller.signal);
    return () => controller.abort();
  });

  async function load(which: AnnouncementTab, signal: AbortSignal) {
    try {
      const [liveList, tabList] = await Promise.all([
        listAnnouncements("live", signal),
        which === "live" ? null : listAnnouncements(which, signal),
      ]);
      now = new Date();
      live = liveList;
      shown = { tab: which, data: tabList ?? liveList };
      loadError = null;
    } catch (error) {
      if (isAbortError(error)) return;
      loadError = isApiError(error) ? error.message : "We could not load the announcements. Try again.";
    }
  }

  function reload() {
    reloads += 1;
  }

  const counts = $derived(live?.counts ?? null);
  const tabs = $derived<TabItem<AnnouncementTab>[]>([
    { value: "live", label: "Live", count: counts?.live, href: BASE },
    { value: "waiting", label: "Waiting", count: counts?.waiting, href: `${BASE}?tab=waiting` },
    { value: "expired", label: "Expired", count: counts?.expired, href: `${BASE}?tab=expired` },
  ]);

  const sources = $derived(permissions?.tickerSources ?? []);
  const canPost = $derived(sources.length > 0);
  const slotsFull = $derived(live !== null && live.slots.used >= live.slots.total);
  // Leads can still send theirs for approval when the ticker is full; it waits for a free slot.
  const blocked = $derived(
    canPost && slotsFull && permissions !== null && sources.every((source) => postsDirectly(permissions, source)),
  );
  const reorderable = $derived(permissions !== null && canReorder(permissions));
  const rows = $derived(shown !== null && shown.tab === tab ? shown.data.announcements : null);

  // The expired tab comes 50 at a time; "Show more" adds the next ones under the list.
  let list = $state<AnnouncementList>();
  let loadingMore = $state(false);
  let moreNotice = $state("");
  const nextBefore = $derived(shown !== null && shown.tab === "expired" && tab === "expired" ? shown.data.nextBefore : null);

  async function showMore() {
    const base = shown;
    if (base === null || base.data.nextBefore === null || loadingMore) return;
    loadingMore = true;
    moreNotice = "";
    try {
      const next = await listAnnouncements("expired", undefined, base.data.nextBefore);
      // A reload or another tab replaced the list while this page loaded.
      if (shown !== base) return;
      const { list: merged, added } = appendPage(base.data, next);
      shown = { tab: "expired", data: merged };
      live = live === null ? null : { ...live, counts: merged.counts };
      moreNotice = addedMessage(added.length, merged.announcements.length, merged.counts.expired);
      await tick();
      // Carry on from the first row that just appeared. With nothing new and the button gone (the
      // last page held only rows already shown), the last row keeps focus off the page body.
      const target = added[0] ?? (merged.nextBefore === null ? merged.announcements.at(-1) : undefined);
      if (target !== undefined) list?.focusRow(target.id);
    } catch (error) {
      toasts.error(failure(error, "We could not load more announcements. Try again."));
      await tick();
      document.querySelector<HTMLElement>("#ticker-show-more")?.focus();
    } finally {
      loadingMore = false;
    }
  }

  function canRepost(announcement: Announcement): boolean {
    return sources.includes(announcement.committee);
  }

  function failure(error: unknown, fallback: string): string {
    return isApiError(error) ? error.message : fallback;
  }

  async function extend(announcement: Announcement) {
    try {
      const updated = await extendAnnouncement(announcement.id, 7);
      toasts.success(`Extended. It now leaves the ticker ${formatDateTimeLong(updated.expiresAt)}.`);
      reload();
    } catch (error) {
      toasts.error(failure(error, "We could not extend it. Try again."));
    }
  }

  // Moves save one at a time, newest last, so a slow answer never undoes a later move.
  let nextOrder: string[] | null = null;
  let savingOrder = false;

  async function saveNextOrder(): Promise<void> {
    const sending = nextOrder;
    if (sending === null) return;
    nextOrder = null;
    const updated = await reorderAnnouncements(sending);
    if (nextOrder !== null) return saveNextOrder();
    live = updated;
    if (tab === "live") shown = { tab: "live", data: updated };
  }

  async function reorder(ids: string[]) {
    nextOrder = ids;
    if (savingOrder) return;
    savingOrder = true;
    try {
      await saveNextOrder();
    } catch (error) {
      nextOrder = null;
      toasts.error(failure(error, "We could not save the new order. Try again."));
      reload();
    } finally {
      savingOrder = false;
    }
  }

  // End now and Delete both ask first.
  let confirming = $state<{ kind: "end" | "delete"; announcement: Announcement } | null>(null);
  let confirmOpen = $state(false);
  let confirmBusy = $state(false);

  function ask(kind: "end" | "delete", announcement: Announcement) {
    confirming = { kind, announcement };
    confirmOpen = true;
  }

  async function confirm() {
    if (confirming === null) return;
    const { kind, announcement } = confirming;
    confirmBusy = true;
    try {
      if (kind === "end") {
        await endAnnouncement(announcement.id);
        toasts.success("Ended. It is off the home page now.");
      } else {
        await deleteAnnouncement(announcement.id);
        toasts.success("Deleted.");
      }
      confirmOpen = false;
      // The row that opened the dialog goes once the list reloads, taking the focus the dialog
      // gives back to its menu button with it. Wait for the new list, then carry on from the
      // page heading. While the dialog was open the page behind it was inert and could not take focus.
      await load(tab, new AbortController().signal);
      await tick();
      focusPageHeading();
    } catch (error) {
      toasts.error(failure(error, kind === "end" ? "We could not end it. Try again." : "We could not delete it. Try again."));
    } finally {
      confirmBusy = false;
    }
  }

  const emptyStates: Record<AnnouncementTab, { title: string; description: string }> = {
    live: {
      title: "Nothing is on the ticker",
      description: "Post an announcement and it scrolls across the home page.",
    },
    waiting: {
      title: "Nothing is waiting for approval",
      description: "Announcements from leads wait here until a director approves them.",
    },
    expired: {
      title: "Nothing has left the ticker yet",
      description: "Announcements that expire or end stay here for 90 days, so you can post them again.",
    },
  };
  const emptyIcons = { live: Megaphone, waiting: Hourglass, expired: Archive } as const;

  const slotsLine = $derived(live === null ? undefined : `${live.slots.used} of ${live.slots.total} slots live`);
</script>

{#snippet livePreview()}
  <LivePreview announcements={live?.announcements ?? null} used={live?.slots.used ?? null} total={live?.slots.total ?? MAX_LIVE} />
{/snippet}

<svelte:head>
  <title>Ticker - ScottyLabs Portal</title>
</svelte:head>

<div class="ticker-page">
  {#if phone.current}{@render livePreview()}{/if}

  <PageHeader
    title="Announcements"
    description={phone.current ? slotsLine : "What scrolls across the front page of scottylabs.org."}
    class="ticker-page__header"
  >
    {#snippet actions()}
      {#if canPost}
        <div class="ticker-page__new">
          <Button
            size={phone.current ? "md" : "lg"}
            icon={Plus}
            href="/portal/ticker/new/"
            disabled={blocked}
            aria-describedby={blocked ? "ticker-slots-full" : undefined}
          >
            {#if phone.current}New<span class="p-sr-only"> announcement</span>{:else}New announcement{/if}
          </Button>
          {#if blocked}
            <p id="ticker-slots-full" class="ticker-page__reason">
              All {MAX_LIVE} slots are in use. End one, or wait for one to expire.
            </p>
          {/if}
        </div>
      {:else if permissions !== null}
        <!-- Leads whose director has not let them post (spec O-2: a lock and the reason). -->
        <p class="ticker-page__locked">
          <Lock size={16} strokeWidth={2} aria-hidden="true" />
          <span>You can&rsquo;t post to the ticker yet. Ask the director of your committee.</span>
        </p>
      {/if}
    {/snippet}
  </PageHeader>

  {#if !phone.current}{@render livePreview()}{/if}

  <Tabs {tabs} value={tab} label="Announcements" />

  {#if loadError !== null}
    <Callout tone="error" role="alert" title="The announcements did not load">
      {loadError}
      {#snippet actions()}<Button variant="outline" size="sm" onclick={reload}>Try again</Button>{/snippet}
    </Callout>
  {:else if rows === null}
    <div class="ticker-page__loading" aria-busy="true" aria-label="Loading announcements">
      {#each { length: 3 }, index (index)}<Skeleton height="56px" radius="10px" />{/each}
    </div>
  {:else if rows.length === 0}
    <div class="p-table-card">
      <EmptyState icon={emptyIcons[tab]} title={emptyStates[tab].title} description={emptyStates[tab].description} />
    </div>
  {:else}
    <AnnouncementList
      bind:this={list}
      {tab}
      announcements={rows}
      {now}
      {highlight}
      {reorderable}
      {canRepost}
      onreorder={(ids) => void reorder(ids)}
      onextend={(announcement) => void extend(announcement)}
      onend={(announcement) => ask("end", announcement)}
      ondelete={(announcement) => ask("delete", announcement)}
    />
    <p class="p-table-footnote">
      Announcements leave the ticker on their own when they expire.
      {#if tab === "live" && reorderable}
        <!-- The cards that replace the table on narrow screens have no handle, only the row menu. -->
        <span class="ticker-page__order ticker-page__order--wide">Drag the handle to change the order, or focus it and use the arrow keys.</span>
        <span class="ticker-page__order ticker-page__order--narrow">Use Move up and Move down in a row&rsquo;s menu to change the order.</span>
      {/if}
      Up to {MAX_LIVE} can be live at once.
    </p>
    {#if nextBefore !== null}
      <div class="ticker-page__more">
        <Button id="ticker-show-more" variant="outline" loading={loadingMore} onclick={() => void showMore()}>
          Show more
        </Button>
        <p class="ticker-page__more-count">
          Showing {rows.length} of {Math.max(counts?.expired ?? 0, rows.length)}
        </p>
      </div>
    {/if}
  {/if}
  <p class="p-sr-only" aria-live="polite">{moreNotice}</p>
</div>

<Dialog
  bind:open={confirmOpen}
  size="sm"
  dismissible={!confirmBusy}
  title={confirming?.kind === "delete" ? "Delete this announcement?" : "End this announcement now?"}
  description={confirming === null
    ? undefined
    : confirming.kind === "delete"
      ? `${quoted(confirming.announcement.message)} will not be published and its approval request is withdrawn. You can't undo this.`
      : `${quoted(confirming.announcement.message)} leaves the home page right away. You can post it again from Expired.`}
>
  {#snippet footer()}
    <Button variant="subtle" disabled={confirmBusy} onclick={() => (confirmOpen = false)}>Cancel</Button>
    <Button variant="danger" loading={confirmBusy} onclick={() => void confirm()}>
      {confirming?.kind === "delete" ? "Delete" : "End now"}
    </Button>
  {/snippet}
</Dialog>

<style>
  .ticker-page {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .ticker-page__new {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
  }
  .ticker-page__reason {
    max-width: 320px;
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
    text-align: right;
  }
  .ticker-page__locked {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    max-width: 280px;
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;

    & :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
  .ticker-page__loading {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .ticker-page :global(.p-table-footnote) {
    margin-top: -12px;
  }
  .ticker-page__order--narrow {
    display: none;
  }
  .ticker-page__more {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .ticker-page__more-count {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  /* Where AnnouncementList swaps its table for cards. */
  @media (max-width: 720px) {
    .ticker-page__order--wide {
      display: none;
    }
    .ticker-page__order--narrow {
      display: inline;
    }
  }
  @media (max-width: 600px) {
    .ticker-page {
      gap: 16px;
    }
    .ticker-page :global(.ticker-page__header) {
      flex-wrap: nowrap;
      align-items: center;
      gap: 12px;
    }
    .ticker-page :global(.ticker-page__header .p-page-header__text) {
      gap: 2px;
    }
    .ticker-page :global(.ticker-page__header .p-page-header__title) {
      font-size: 1.25rem;
      font-weight: 500;
      line-height: 28px;
      letter-spacing: 0;
    }
    .ticker-page :global(.ticker-page__header .p-page-header__description) {
      color: var(--p-text-tertiary);
      font-family: var(--p-mono);
      font-size: 0.75rem;
      line-height: 16px;
    }
    .ticker-page__reason {
      max-width: 168px;
    }
    .ticker-page__locked {
      max-width: 168px;
      font-size: 0.75rem;
      line-height: 16px;
    }
  }
</style>
