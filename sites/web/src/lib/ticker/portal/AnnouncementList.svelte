<script lang="ts">
  // The announcements in one tab: a table on wide screens and cards on phones. Live rows can be
  // dragged by their handle to change the loop's order, or moved with the arrow keys on the
  // handle and with Move up and Move down in the row menu.
  import CalendarPlus from "@lucide/svelte/icons/calendar-plus";
  import CircleStop from "@lucide/svelte/icons/circle-stop";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import MoveDown from "@lucide/svelte/icons/arrow-down";
  import MoveUp from "@lucide/svelte/icons/arrow-up";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Repeat from "@lucide/svelte/icons/repeat-2";
  import Trash from "@lucide/svelte/icons/trash";
  import { tick } from "svelte";
  import type { Announcement, AnnouncementTab } from "../../api/types.ts";
  import { formatAbsolute, formatRelative, formatShortDate } from "../../portal/dates.ts";
  import { Avatar, Badge, Button, CommitteeChip, Menu, StatusDot, type MenuItem } from "../../portal/ui/index.ts";
  import { linkHost } from "../../site/links.ts";
  import { leavesLabel, quoted, timeLeft } from "../format.ts";
  import { kindIcon, kindLabel } from "../kinds.ts";
  import TimeLeft from "./TimeLeft.svelte";

  let {
    tab,
    announcements,
    now,
    highlight = null,
    reorderable = false,
    canRepost,
    onreorder,
    onextend,
    onend,
    ondelete,
  }: {
    tab: AnnouncementTab;
    announcements: readonly Announcement[];
    now: Date;
    /** The announcement just published or sent, shown highlighted. */
    highlight?: string | null;
    reorderable?: boolean;
    canRepost: (announcement: Announcement) => boolean;
    onreorder: (ids: string[], moved: Announcement) => void;
    onextend: (announcement: Announcement) => void;
    onend: (announcement: Announcement) => void;
    ondelete: (announcement: Announcement) => void;
  } = $props();

  const uid = $props.id();
  const handleHelpId = `reorder-help-${uid}`;

  // The order on screen, which drag and the arrow keys change before it is saved.
  let order = $derived(announcements.map((announcement) => announcement.id));
  const byId = $derived(new Map(announcements.map((announcement) => [announcement.id, announcement])));
  const rows = $derived(order.map((id) => byId.get(id)).filter((row) => row !== undefined));
  const canDrag = $derived(reorderable && tab === "live" && rows.length > 1);

  // Bring the row just published into view once; on phones it often lands below the fold.
  let shownHighlight: string | null = null;
  $effect(() => {
    const id = highlight;
    if (id === null || id === shownHighlight || !byId.has(id)) return;
    shownHighlight = id;
    void tick().then(() => {
      const target = [...document.querySelectorAll<HTMLElement>("[data-highlighted]")].find(
        (element) => element.offsetParent !== null,
      );
      if (target === undefined) return;
      const box = target.getBoundingClientRect();
      // Leave room for the phone tab bar and the toast above it.
      if (box.top >= 0 && box.bottom <= window.innerHeight - 140) return;
      const smooth = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      target.scrollIntoView({ block: "center", behavior: smooth ? "smooth" : "auto" });
    });
  });

  let dragging = $state<string | null>(null);
  let dragStart: string[] = [];
  let body = $state<HTMLElement>();
  let notice = $state("");

  function moved(id: string, to: number): string[] {
    const next = order.filter((other) => other !== id);
    next.splice(to, 0, id);
    return next;
  }

  function commit(next: string[], id: string) {
    const announcement = byId.get(id);
    if (announcement === undefined || next.join() === announcements.map((a) => a.id).join()) return;
    notice = `Moved ${quoted(announcement.message)} to position ${next.indexOf(id) + 1} of ${next.length}.`;
    onreorder(next, announcement);
  }

  async function moveBy(id: string, delta: number) {
    const from = order.indexOf(id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= order.length) return;
    order = moved(id, to);
    commit(order, id);
    // Keyed rows move in the DOM, which can drop focus; keep it on the handle that moved.
    await tick();
    body?.querySelector<HTMLElement>(`[data-handle="${id}"]`)?.focus();
  }

  function onHandleKeydown(event: KeyboardEvent, id: string) {
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      void moveBy(id, event.key === "ArrowUp" ? -1 : 1);
    }
  }

  function onHandlePointerDown(event: PointerEvent, id: string) {
    if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    dragging = id;
    dragStart = order;
  }

  function onHandlePointerMove(event: PointerEvent) {
    const id = dragging;
    if (id === null || body === undefined) return;
    const rowElements = [...body.querySelectorAll<HTMLElement>("tr[data-row]")];
    let target = rowElements.length - 1;
    for (const [index, row] of rowElements.entries()) {
      const box = row.getBoundingClientRect();
      if (event.clientY < box.top + box.height / 2) {
        target = index;
        break;
      }
    }
    if (order.indexOf(id) !== target) order = moved(id, target);
  }

  function onHandlePointerUp() {
    const id = dragging;
    dragging = null;
    if (id !== null && order.join() !== dragStart.join()) commit(order, id);
  }

  function menuItems(announcement: Announcement, index: number): (MenuItem | "separator")[] {
    const items: (MenuItem | "separator")[] = [];
    if (announcement.canEdit) {
      items.push({ label: "Edit", icon: Pencil, href: `/portal/ticker/${announcement.id}/` });
    }
    if (tab === "live" && announcement.canManage) {
      items.push({ label: "Extend by 1 week", icon: CalendarPlus, onSelect: () => onextend(announcement) });
    }
    if (canDrag) {
      items.push(
        { label: "Move up", icon: MoveUp, disabled: index === 0, onSelect: () => void moveBy(announcement.id, -1) },
        {
          label: "Move down",
          icon: MoveDown,
          disabled: index === rows.length - 1,
          onSelect: () => void moveBy(announcement.id, 1),
        },
      );
    }
    if (tab === "live" && announcement.canManage) {
      if (items.length > 0) items.push("separator");
      items.push({ label: "End now", icon: CircleStop, danger: true, onSelect: () => onend(announcement) });
    }
    if (tab === "waiting" && (announcement.canEdit || announcement.canManage)) {
      if (items.length > 0) items.push("separator");
      items.push({ label: "Delete", icon: Trash, danger: true, onSelect: () => ondelete(announcement) });
    }
    return items;
  }

  function leftLabel(announcement: Announcement): string {
    return announcement.endedAt === null
      ? `Expired ${formatShortDate(announcement.expiresAt, now)}`
      : `Ended early ${formatShortDate(announcement.endedAt, now)}`;
  }
</script>

{#snippet message(announcement: Announcement)}
  {@const host = linkHost(announcement.linkUrl)}
  <span class="list__message">
    <span class="p-table__title">
      {announcement.message}
      {#if announcement.id === highlight}<Badge tone="info">new</Badge>{/if}
    </span>
    <span class="p-table__subtitle">{host ?? "No link"}</span>
  </span>
{/snippet}

{#snippet poster(announcement: Announcement)}
  {#if announcement.createdBy}
    <span class="list__poster">
      <Avatar initials={announcement.createdBy.initials} committee={announcement.committee} size="sm" />
      <span class="p-table__muted">{announcement.createdBy.name}</span>
    </span>
  {:else}
    <span class="p-table__muted">Not recorded</span>
  {/if}
{/snippet}

{#snippet status(announcement: Announcement)}
  {#if announcement.state === "sent_back"}
    <span class="list__status">
      <StatusDot tone="warning" size="sm" label="Sent back" />
      {#if announcement.approval?.note}
        <span class="list__note">
          &ldquo;{announcement.approval.note}&rdquo;{#if announcement.approval.decidedBy}&nbsp;&ndash; {announcement.approval.decidedBy.name}{/if}
        </span>
      {/if}
    </span>
  {:else}
    <StatusDot tone="info" size="sm" label="Waiting for approval" />
  {/if}
{/snippet}

{#snippet actions(announcement: Announcement, index: number)}
  {#if tab === "expired"}
    {#if canRepost(announcement)}
      <Button variant="outline" size="sm" icon={Repeat} href={`/portal/ticker/new/?from=${announcement.id}`}>Post again</Button>
    {/if}
  {:else}
    {@const items = menuItems(announcement, index)}
    {#if items.length > 0}
      <Menu {items} label={`Actions for ${announcement.message}`} width={220}>
        {#snippet trigger(props)}
          <button {...props} type="button" class="list__menu-button" aria-label={`Actions for ${quoted(announcement.message)}`}>
            <Ellipsis size={18} strokeWidth={1.75} aria-hidden="true" />
          </button>
        {/snippet}
      </Menu>
    {/if}
  {/if}
{/snippet}

<div class="list">
  <div class="p-table-card list__table">
    <table class="p-table">
      <thead>
        <tr>
          {#if canDrag}<th scope="col" class="list__handle-col"><span class="p-sr-only">Order</span></th>{/if}
          <th scope="col" class="list__icon-col"><span class="p-sr-only">Type</span></th>
          <th scope="col">Announcement</th>
          <th scope="col">Committee</th>
          {#if tab === "live"}
            <th scope="col">Expires</th>
          {:else if tab === "waiting"}
            <th scope="col">Status</th>
            <th scope="col">Leaves</th>
          {:else}
            <th scope="col">Left the ticker</th>
          {/if}
          <th scope="col">Posted by</th>
          <th scope="col"><span class="p-sr-only">Actions</span></th>
        </tr>
      </thead>
      <tbody bind:this={body}>
        {#each rows as announcement, index (announcement.id)}
          {@const Icon = kindIcon(announcement.kind)}
          <tr
            data-row
            data-highlighted={announcement.id === highlight ? "" : undefined}
            class={[announcement.id === highlight && "p-table__row--highlight", dragging === announcement.id && "list__row--dragging"]}
          >
            {#if canDrag}
              <td class="list__handle-cell">
                <button
                  type="button"
                  class="list__handle"
                  data-handle={announcement.id}
                  aria-label={`Move ${quoted(announcement.message)}, position ${index + 1} of ${rows.length}`}
                  aria-describedby={handleHelpId}
                  onkeydown={(event) => onHandleKeydown(event, announcement.id)}
                  onpointerdown={(event) => onHandlePointerDown(event, announcement.id)}
                  onpointermove={onHandlePointerMove}
                  onpointerup={onHandlePointerUp}
                  onpointercancel={onHandlePointerUp}
                >
                  <GripVertical size={16} strokeWidth={1.75} aria-hidden="true" />
                </button>
              </td>
            {/if}
            <td class="list__icon-cell">
              <Icon size={16} strokeWidth={1.75} aria-hidden="true" />
              <span class="p-sr-only">{kindLabel(announcement.kind)}</span>
            </td>
            <td>{@render message(announcement)}</td>
            <td><CommitteeChip committee={announcement.committee} /></td>
            {#if tab === "live"}
              <td><TimeLeft {announcement} {now} /></td>
            {:else if tab === "waiting"}
              <td>{@render status(announcement)}</td>
              <td>
                <span class="list__when">{formatAbsolute(announcement.expiresAt, now)}</span>
              </td>
            {:else}
              <td>
                <span class="list__when">{leftLabel(announcement)}</span>
                <span class="list__when-sub">{formatRelative(announcement.endedAt ?? announcement.expiresAt, now)}</span>
              </td>
            {/if}
            <td>{@render poster(announcement)}</td>
            <td class="p-table__actions">{@render actions(announcement, index)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <ul class="list__cards">
    {#each rows as announcement, index (announcement.id)}
      {@const soon = tab === "live" && timeLeft(announcement, now).soon}
      <li
        class={["list__card", announcement.id === highlight && "list__card--highlight"]}
        data-highlighted={announcement.id === highlight ? "" : undefined}
      >
        <div class="list__card-top">
          <p class="list__card-message">
            {announcement.message}
            {#if announcement.id === highlight}<Badge tone="info">new</Badge>{/if}
          </p>
          {@render actions(announcement, index)}
        </div>
        <div class="list__card-meta">
          <CommitteeChip committee={announcement.committee} />
          {#if tab === "live"}
            <span class={["list__card-when", soon && "list__card-when--soon"]}>{leavesLabel(announcement.expiresAt, now)}</span>
          {:else if tab === "waiting"}
            {@render status(announcement)}
          {:else}
            <span class="list__card-when">{leftLabel(announcement)}</span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  <p id={handleHelpId} class="p-sr-only">Press the up or down arrow to move it in the loop.</p>
  <p class="p-sr-only" aria-live="polite">{notice}</p>
</div>

<style>
  .list__table {
    display: block;
  }
  /* Row menus open below their row; phones get the cards, so the table never needs to scroll. */
  .list .p-table-card.list__table {
    overflow: visible;
  }
  .list__cards {
    display: none;
    flex-direction: column;
    gap: 12px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  @media (max-width: 720px) {
    .list__table {
      display: none;
    }
    .list__cards {
      display: flex;
    }
  }

  .list__handle-col,
  .list__icon-col {
    width: 1px;
  }
  .p-table .list__handle-cell {
    padding-right: 0;
  }
  .list__handle {
    display: grid;
    place-items: center;
    width: 28px;
    height: 32px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text-secondary);
    cursor: grab;
    touch-action: none;

    &:hover {
      background: var(--p-surface-muted);
      color: var(--p-text);
    }
    &:active {
      cursor: grabbing;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .list__row--dragging {
    position: relative;
    background: var(--p-surface-muted);
    box-shadow: var(--p-shadow-card);
  }
  .list__icon-cell {
    color: var(--p-text);
    line-height: 0;
  }
  .list__message {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;

    & .p-table__title {
      display: inline-flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 8px;
      overflow-wrap: anywhere;
    }
  }
  .list__poster {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 0.75rem;
    white-space: nowrap;
  }
  .list__status {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .list__note {
    max-width: 280px;
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .list__when {
    display: block;
    color: var(--p-text);
    font-size: 0.8125rem;
    line-height: 18px;
    white-space: nowrap;
  }
  .list__when-sub {
    display: block;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .list__menu-button {
    display: inline-grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--p-radius-pill);
    color: var(--p-text);
    cursor: pointer;

    &:hover,
    &[aria-expanded="true"] {
      background: var(--p-surface-strong);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }

  .list__card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 14px 14px 16px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
  }
  .list__card--highlight {
    background: var(--p-info-surface);
  }
  .list__card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }
  .list__card-message {
    margin: 4px 0 0;
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 18px;
    overflow-wrap: anywhere;
  }
  .list__card-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
  }
  .list__card-when {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .list__card-when--soon {
    color: var(--p-warning-text);
  }
</style>
