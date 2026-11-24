<script lang="ts" generics="T extends string">
  import type { SelectOption } from "./types.ts";

  let {
    value = $bindable(),
    options,
    label,
    size = "md",
    fullWidth = false,
    onchange,
    class: className,
  }: {
    value?: T;
    options: readonly SelectOption<T>[];
    /** Names the group for assistive technology. */
    label: string;
    size?: "sm" | "md";
    fullWidth?: boolean;
    onchange?: (value: T) => void;
    class?: string;
  } = $props();

  const buttons: HTMLButtonElement[] = $state([]);

  // The checked option takes focus; with none checked, the first enabled one does.
  const focusIndex = $derived.by(() => {
    const checked = options.findIndex((option) => option.value === value);
    return checked >= 0 ? checked : options.findIndex((option) => !option.disabled);
  });

  function select(index: number) {
    const option = options[index];
    if (option === undefined || option.disabled) return;
    if (value !== option.value) {
      value = option.value;
      onchange?.(option.value);
    }
  }

  function step(from: number, direction: 1 | -1): number {
    for (let i = 1; i <= options.length; i++) {
      const index = (from + direction * i + options.length) % options.length;
      if (!options[index]?.disabled) return index;
    }
    return from;
  }

  function onkeydown(event: KeyboardEvent, index: number) {
    let next: number | null = null;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = step(index, 1);
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = step(index, -1);
    else if (event.key === "Home") next = step(-1, 1);
    else if (event.key === "End") next = step(options.length, -1);
    if (next === null) return;
    event.preventDefault();
    select(next);
    buttons[next]?.focus();
  }
</script>

<div
  class={["p-segmented", `p-segmented--${size}`, fullWidth && "p-segmented--full", className]}
  role="radiogroup"
  aria-label={label}
>
  {#each options as option, index (option.value)}
    <button
      bind:this={buttons[index]}
      type="button"
      role="radio"
      class="p-segmented__option"
      aria-checked={option.value === value ? "true" : "false"}
      tabindex={index === focusIndex ? 0 : -1}
      disabled={option.disabled}
      onclick={() => select(index)}
      onkeydown={(event) => onkeydown(event, index)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .p-segmented {
    box-sizing: border-box;
    display: inline-flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--p-radius);
    background: var(--p-surface-strong);
    font-family: var(--p-font);
  }
  .p-segmented--full {
    display: flex;
    width: 100%;
  }
  .p-segmented--full .p-segmented__option {
    flex: 1;
  }
  .p-segmented__option {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0 12px;
    border-radius: 7px;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--p-duration) var(--p-ease),
      color var(--p-duration) var(--p-ease);

    &:hover:not(:disabled) {
      color: var(--p-text);
    }
    &[aria-checked="true"] {
      background: var(--p-surface);
      color: var(--p-text);
      box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: not-allowed;
    }
  }
  .p-segmented--md .p-segmented__option {
    height: 32px;
  }
  .p-segmented--sm .p-segmented__option {
    height: 26px;
    padding: 0 10px;
    font-size: 0.75rem;
  }
</style>
