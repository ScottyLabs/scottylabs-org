<script lang="ts">
  import type { TickerSource } from "../../api/types.ts";
  import { COMMITTEES } from "../committees.ts";

  let {
    committee,
    size = "md",
    count,
    selectable = false,
    selected = false,
    disabled = false,
    onclick,
    label,
    class: className,
  }: {
    committee: TickerSource;
    size?: "sm" | "md";
    /** A number after the name, for filters. */
    count?: number;
    /** Renders a toggle button; `selected` sets its pressed state. */
    selectable?: boolean;
    selected?: boolean;
    disabled?: boolean;
    onclick?: (event: MouseEvent) => void;
    /** Overrides the committee name. */
    label?: string;
    class?: string;
  } = $props();

  const style = $derived(COMMITTEES[committee]);
  const text = $derived(label ?? style.name);
  const classes = $derived([
    "p-chip",
    `p-chip--${size}`,
    selectable && "p-chip--selectable",
    selected && "p-chip--selected",
    className,
  ]);
</script>

{#snippet content()}
  <span class="p-chip__dot" aria-hidden="true"></span>
  <span>{text}</span>
  {#if count !== undefined}<span class="p-chip__count">{count}</span>{/if}
{/snippet}

{#if selectable}
  <button
    type="button"
    class={classes}
    style:--chip-color={style.color}
    style:--chip-tint={style.tint}
    aria-pressed={selected ? "true" : "false"}
    {disabled}
    {onclick}
  >
    {@render content()}
  </button>
{:else}
  <span class={classes} style:--chip-color={style.color} style:--chip-tint={style.tint}>
    {@render content()}
  </span>
{/if}

<style>
  .p-chip {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    flex: none;
    gap: 6px;
    border: 1px solid transparent;
    border-radius: var(--p-radius-pill);
    background: var(--chip-tint);
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 500;
    white-space: nowrap;
  }
  .p-chip--md {
    height: 22px;
    padding: 0 9px 0 8px;
    font-size: 0.75rem;
    line-height: 14px;
  }
  .p-chip--sm {
    height: 18px;
    padding: 0 7px 0 6px;
    gap: 5px;
    font-size: 0.6875rem;
    line-height: 12px;
  }
  .p-chip__dot {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
    background: var(--chip-color);
  }
  .p-chip__count {
    color: #555555;
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }
  .p-chip--selectable {
    cursor: pointer;
    background: var(--p-surface);
    border-color: var(--p-border);
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease);

    &.p-chip--md {
      height: 30px;
      padding: 0 11px 0 10px;
    }
    &:hover:not(:disabled) {
      background: var(--chip-tint);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: not-allowed;
    }
  }
  .p-chip--selectable.p-chip--selected {
    background: var(--chip-tint);
    border-color: var(--p-text);
  }
</style>
