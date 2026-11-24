<script lang="ts">
  // Expires (spec TK-5): Tomorrow, In 3 days, In 1 week, In 2 weeks, or Pick a date, and a
  // sentence that says exactly when the announcement leaves.
  import CalendarClock from "@lucide/svelte/icons/calendar-clock";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Clock from "@lucide/svelte/icons/clock";
  import { onMount, tick } from "svelte";
  import Sheet from "../../portal/ui/Sheet.svelte";
  import { EXPIRY_PRESETS, expirySentence, presetExpiry, presetFor, shortDateTime, type ExpiryPreset } from "../format.ts";
  import DatePicker from "./DatePicker.svelte";

  let {
    value = $bindable(),
    picking = $bindable(false),
    now,
    error = null,
  }: {
    /** The chosen expiry as an RFC 3339 string, or null before one is chosen. */
    value: string | null;
    /** True while the date picker is open; nothing is set until it closes with a date. */
    picking?: boolean;
    /** The current time, kept fresh by the page so the presets and the sentence follow the clock. */
    now: Date;
    error?: string | null;
  } = $props();

  const uid = $props.id();
  const labelId = `expiry-label-${uid}`;
  const sentenceId = `expiry-sentence-${uid}`;
  const errorId = `expiry-error-${uid}`;
  const popoverId = `expiry-picker-${uid}`;

  const preset = $derived(presetFor(value, now));
  const custom = $derived(value !== null && preset === null);
  const describedBy = $derived(error ? `${sentenceId} ${errorId}` : sentenceId);

  let phone = $state(false);
  let pickButton = $state<HTMLElement>();
  let options = $state<HTMLElement>();
  let popover = $state<HTMLElement>();
  let popoverLeft = $state(0);

  onMount(() => {
    const query = window.matchMedia("(max-width: 600px)");
    phone = query.matches;
    const update = () => (phone = query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  function choose(option: ExpiryPreset) {
    picking = false;
    // Counted from this moment, not from when the page opened.
    value = presetExpiry(option, new Date());
  }

  /** Takes the keyboard to the field, for example when it has a problem: the open calendar, the chosen option or Pick a date. */
  export function focus() {
    const target =
      popover?.querySelector<HTMLElement>("[data-day][tabindex='0']") ??
      options?.querySelector<HTMLElement>("[aria-pressed='true']") ??
      pickButton;
    target?.focus();
  }

  async function openPicker() {
    if (picking) {
      closePicker(true);
      return;
    }
    picking = true;
    if (phone) return;
    await tick();
    place();
    popover?.querySelector<HTMLElement>("[data-day][tabindex='0']")?.focus();
  }

  function closePicker(returnFocus: boolean) {
    picking = false;
    if (returnFocus) pickButton?.focus();
  }

  function set(expiresAt: string) {
    value = expiresAt;
    closePicker(true);
  }

  /** Lines the popover's right edge up with the Pick a date button, inside the field. */
  function place() {
    if (pickButton === undefined || popover === undefined) return;
    const right = pickButton.offsetLeft + pickButton.offsetWidth;
    popoverLeft = Math.max(0, right - popover.offsetWidth);
  }

  function onPopoverKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      closePicker(true);
    }
  }

  function onDocumentPointerDown(event: PointerEvent) {
    const target = event.target;
    if (!(target instanceof Node)) return;
    if (popover?.contains(target) === true || pickButton?.contains(target) === true) return;
    closePicker(false);
  }

  $effect(() => {
    if (!picking || phone) return;
    document.addEventListener("pointerdown", onDocumentPointerDown, true);
    return () => document.removeEventListener("pointerdown", onDocumentPointerDown, true);
  });

  const sentence = $derived(
    picking || value === null ? "Pick the day and time this should leave the ticker." : expirySentence(value, now),
  );
</script>

<div class="expiry">
  <span id={labelId} class="expiry__label">Expires</span>
  <div class="expiry__options" role="group" aria-labelledby={labelId} bind:this={options}>
    {#each EXPIRY_PRESETS as option (option.value)}
      <button
        type="button"
        class={["expiry__option", !picking && preset === option.value && "expiry__option--selected"]}
        aria-pressed={!picking && preset === option.value ? "true" : "false"}
        aria-describedby={describedBy}
        onclick={() => choose(option.value)}
      >
        {option.label}
      </button>
    {/each}
    <button
      type="button"
      class={["expiry__option", (picking || custom) && "expiry__option--selected"]}
      aria-haspopup="dialog"
      aria-expanded={picking ? "true" : "false"}
      aria-controls={phone ? undefined : popoverId}
      aria-describedby={describedBy}
      bind:this={pickButton}
      onclick={() => void openPicker()}
    >
      <CalendarClock size={16} strokeWidth={1.75} aria-hidden="true" />
      {#if custom && value !== null && !picking}{shortDateTime(value, now)}{:else}Pick a date{/if}
    </button>

    {#if picking && !phone}
      <div
        id={popoverId}
        class="expiry__popover"
        role="dialog"
        tabindex="-1"
        aria-label="When should it leave the ticker?"
        style:left="{popoverLeft}px"
        bind:this={popover}
        onkeydown={onPopoverKeydown}
      >
        <DatePicker {value} {now} onset={set} oncancel={() => closePicker(true)} />
      </div>
    {/if}
  </div>

  <p id={sentenceId} class="expiry__sentence" aria-live="polite">
    <Clock size={16} strokeWidth={1.75} aria-hidden="true" />
    <span>{sentence}</span>
  </p>
  <div id={errorId} class="expiry__error" aria-live="polite">
    {#if error}<CircleAlert size={14} strokeWidth={2} aria-hidden="true" /><span>{error}</span>{/if}
  </div>
</div>

{#if phone}
  <Sheet bind:open={picking} title="When should it leave the ticker?">
    <DatePicker {value} {now} layout="sheet" onset={set} />
  </Sheet>
{/if}

<style>
  .expiry {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .expiry__label {
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .expiry__options {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .expiry__option {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-sm);
    background: var(--p-surface);
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    white-space: nowrap;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover:not(.expiry__option--selected) {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .expiry__option--selected {
    border-color: var(--p-inverse);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
  }
  .expiry__popover {
    position: absolute;
    bottom: calc(100% + 8px);
    z-index: 40;
    box-sizing: border-box;
    width: 318px;
    padding: 16px 18px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-pop);
  }
  .expiry__sentence {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    padding: 10px 14px;
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;

    & :global(svg) {
      flex: none;
    }
  }
  .expiry__error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;

    &:empty {
      display: none;
    }
    & :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
</style>
