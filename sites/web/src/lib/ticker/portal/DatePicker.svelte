<script lang="ts">
  // A month calendar and a time, in Pittsburgh time, limited to the next 30 days. Shown in a
  // popover on wide screens and in a bottom sheet on phones.
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Clock from "@lucide/svelte/icons/clock";
  import { tick } from "svelte";
  import { formatDay, formatTime, toZonedInputValue } from "../../portal/dates.ts";
  import Button from "../../portal/ui/Button.svelte";
  import { addDays, expiryProblem, latestExpiry, presetFor, zonedDay, zonedInstant } from "../format.ts";

  let {
    value,
    now,
    layout = "popover",
    onset,
    oncancel,
  }: {
    /** The current expiry, which the picker starts from. */
    value: string | null;
    now: Date;
    layout?: "popover" | "sheet";
    onset: (expiresAt: string) => void;
    oncancel?: () => void;
  } = $props();

  const uid = $props.id();
  const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
  const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

  const first = $derived(zonedDay(now));
  const latest = $derived(latestExpiry(now));
  const last = $derived(zonedDay(latest));

  function initialDay(): string {
    if (value !== null) return zonedDay(value);
    return addDays(zonedDay(now), 7);
  }

  let day = $state(initialDay());
  /** 9:00 AM, unless a date and time were picked before; presets end at 11:59 PM, which is rarely wanted here. */
  function initialTime(): string {
    return value === null || presetFor(value, now) !== null ? "09:00" : toZonedInputValue(value).slice(11, 16);
  }

  let time = $state(initialTime());
  let focusDay = $state(initialDay());
  let error = $state<string | null>(null);

  let view = $state(initialDay().slice(0, 7));
  const viewYear = $derived(Number(view.slice(0, 4)));
  const viewMonth = $derived(Number(view.slice(5, 7)));
  const monthLabel = $derived(`${MONTHS[viewMonth - 1]} ${viewYear}`);

  let grid = $state<HTMLElement>();

  interface Cell {
    day: string;
    date: number;
    inMonth: boolean;
    enabled: boolean;
  }

  const weeks = $derived.by(() => {
    const start = `${view}-01`;
    const offset = new Date(`${start}T00:00:00Z`).getUTCDay();
    const daysInMonth = new Date(Date.UTC(viewYear, viewMonth, 0)).getUTCDate();
    const rows = Math.ceil((offset + daysInMonth) / 7);
    const cells: Cell[] = [];
    for (let index = 0; index < rows * 7; index++) {
      const cellDay = addDays(start, index - offset);
      const inMonth = cellDay.slice(0, 7) === view;
      cells.push({
        day: cellDay,
        date: Number(cellDay.slice(8, 10)),
        inMonth,
        enabled: inMonth && cellDay >= first && cellDay <= last,
      });
    }
    return Array.from({ length: rows }, (_, row) => cells.slice(row * 7, row * 7 + 7));
  });

  /** The one day the Tab key reaches: the focused day when it is in view, else the first open day. */
  const tabStop = $derived.by(() => {
    const open = weeks.flat().filter((cell) => cell.enabled);
    return open.find((cell) => cell.day === focusDay)?.day ?? open.find((cell) => cell.day === day)?.day ?? open[0]?.day;
  });

  const canGoBack = $derived(view > first.slice(0, 7));
  const canGoForward = $derived(view < last.slice(0, 7));

  function shiftMonth(delta: number) {
    const date = new Date(Date.UTC(viewYear, viewMonth - 1 + delta, 1));
    view = date.toISOString().slice(0, 7);
  }

  function clampDay(target: string): string {
    if (target < first) return first;
    if (target > last) return last;
    return target;
  }

  async function moveFocus(target: string) {
    focusDay = clampDay(target);
    view = focusDay.slice(0, 7);
    await tick();
    grid?.querySelector<HTMLElement>(`[data-day="${focusDay}"]`)?.focus();
  }

  function onDayKeydown(event: KeyboardEvent) {
    const weekday = new Date(`${focusDay}T00:00:00Z`).getUTCDay();
    const moves: Record<string, number> = {
      ArrowLeft: -1,
      ArrowRight: 1,
      ArrowUp: -7,
      ArrowDown: 7,
      Home: -weekday,
      End: 6 - weekday,
      PageUp: -30,
      PageDown: 30,
    };
    const delta = moves[event.key];
    if (delta === undefined) return;
    event.preventDefault();
    void moveFocus(addDays(focusDay, delta));
  }

  function choose(cell: Cell) {
    day = cell.day;
    focusDay = cell.day;
    error = null;
  }

  function label(cell: Cell): string {
    const date = new Date(`${cell.day}T12:00:00Z`);
    return `${WEEKDAYS[date.getUTCDay()]}, ${MONTHS[date.getUTCMonth()]} ${cell.date}`;
  }

  function submit() {
    const expiresAt = zonedInstant(day, time);
    const problem = time === "" ? "Pick a time of day." : expiryProblem(expiresAt, new Date());
    if (problem !== null || expiresAt === null) {
      error = problem ?? "Pick a day and a time.";
      return;
    }
    onset(expiresAt);
  }
</script>

<div class={["picker", `picker--${layout}`]}>
  <div class="picker__month">
    <button type="button" class="picker__nav" aria-label="Previous month" disabled={!canGoBack} onclick={() => shiftMonth(-1)}>
      <ChevronLeft size={16} strokeWidth={2} aria-hidden="true" />
    </button>
    <span class="picker__month-label" id={`picker-month-${uid}`} aria-live="polite">{monthLabel}</span>
    <button type="button" class="picker__nav" aria-label="Next month" disabled={!canGoForward} onclick={() => shiftMonth(1)}>
      <ChevronRight size={16} strokeWidth={2} aria-hidden="true" />
    </button>
  </div>

  <table class="picker__grid" aria-labelledby={`picker-month-${uid}`} bind:this={grid}>
    <thead>
      <tr>
        {#each WEEKDAYS as weekday (weekday)}
          <th scope="col" abbr={weekday}>{weekday.slice(0, 1)}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each weeks as week, row (row)}
        <tr>
          {#each week as cell (cell.day)}
            <td>
              {#if cell.inMonth}
                <button
                  type="button"
                  class={["picker__day", cell.day === day && "picker__day--selected", cell.day === first && "picker__day--today"]}
                  data-day={cell.day}
                  aria-label={label(cell)}
                  aria-pressed={cell.day === day ? "true" : "false"}
                  aria-current={cell.day === first ? "date" : undefined}
                  disabled={!cell.enabled}
                  tabindex={cell.day === tabStop ? 0 : -1}
                  onclick={() => choose(cell)}
                  onfocus={() => (focusDay = cell.day)}
                  onkeydown={onDayKeydown}
                >
                  {cell.date}
                </button>
              {:else}
                <span class="picker__day picker__day--outside" aria-hidden="true">{cell.date}</span>
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>

  <div class="picker__time">
    <label class="picker__at" for={`picker-time-${uid}`}>At</label>
    <span class="picker__time-input">
      <Clock size={16} strokeWidth={1.75} aria-hidden="true" />
      <input
        id={`picker-time-${uid}`}
        type="time"
        step="60"
        bind:value={time}
        aria-describedby={`picker-zone-${uid}`}
        oninput={() => (error = null)}
      />
    </span>
    <span class="picker__zone" id={`picker-zone-${uid}`}>Eastern time</span>
  </div>

  <p class="picker__limit">
    The latest you can pick is {formatDay(latest, now)} at {formatTime(latest)}, 30 days from now.
  </p>
  {#if error !== null}<p class="picker__error" role="alert">{error}</p>{/if}

  <div class="picker__actions">
    {#if layout === "popover"}
      <Button variant="subtle" size="sm" onclick={() => oncancel?.()}>Cancel</Button>
      <Button size="sm" onclick={submit}>Set expiry</Button>
    {:else}
      <Button size="lg" fullWidth onclick={submit}>Set expiry</Button>
    {/if}
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 12px;
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .picker__month {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .picker__month-label {
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .picker__nav {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text);

    &:hover:not(:disabled) {
      background: var(--p-surface-muted);
    }
    &:disabled {
      color: var(--p-text-disabled);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }
  .picker__grid {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;

    & th {
      padding: 4px 0 6px;
      color: var(--p-text-tertiary);
      font-size: 0.75rem;
      font-weight: 400;
      line-height: 16px;
      text-align: center;
    }
    & td {
      padding: 1px 0;
      text-align: center;
    }
  }
  .picker__day {
    display: inline-grid;
    place-items: center;
    width: 38px;
    height: 36px;
    border-radius: 18px;
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
    font-variant-numeric: tabular-nums;
    cursor: pointer;

    &:hover:not(:disabled):not(.picker__day--selected) {
      background: var(--p-surface-muted);
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: default;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }
  .picker__day--today:not(.picker__day--selected) {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .picker__day--selected {
    background: var(--p-inverse);
    color: var(--p-on-inverse);
  }
  .picker__day--outside {
    color: var(--p-text-disabled);
    cursor: default;
  }
  .picker__time {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .picker__at {
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .picker__time-input {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-sm);
    background: var(--p-surface);

    &:focus-within {
      outline: 2px solid var(--p-focus);
      outline-offset: -1px;
      border-color: var(--p-focus);
    }
    & input {
      border: 0;
      background: transparent;
      color: var(--p-text);
      font: inherit;
      font-size: 0.875rem;
      outline: none;
    }
    /* The field already shows a clock; Chrome would add a second one. */
    & input::-webkit-calendar-picker-indicator {
      display: none;
    }
  }
  .picker__zone,
  .picker__limit {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .picker__limit,
  .picker__error {
    margin: 0;
  }
  .picker__error {
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .picker__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
