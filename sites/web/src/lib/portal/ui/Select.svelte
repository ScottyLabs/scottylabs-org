<script lang="ts" generics="T extends string">
  import type { HTMLSelectAttributes } from "svelte/elements";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { getFieldContext } from "./field-context.ts";
  import type { SelectOption } from "./types.ts";

  type Props = Omit<HTMLSelectAttributes, "value" | "children" | "size"> & {
    value?: T;
    options: readonly SelectOption<T>[];
    /** A first, empty choice such as "Choose a committee". */
    placeholder?: string;
    invalid?: boolean;
    size?: "sm" | "md";
  };

  let {
    value = $bindable(),
    options,
    placeholder,
    invalid,
    size = "md",
    id,
    class: className,
    ...rest
  }: Props = $props();

  const field = getFieldContext();
  const isInvalid = $derived(invalid ?? field?.invalid ?? false);
</script>

<div class={["p-select", `p-select--${size}`, isInvalid && "p-select--invalid", className]}>
  <select
    bind:value
    {...rest}
    id={id ?? field?.id}
    aria-describedby={rest["aria-describedby"] ?? field?.describedBy}
    aria-invalid={isInvalid ? "true" : undefined}
    required={rest.required ?? field?.required}
  >
    {#if placeholder !== undefined}
      <option value="" disabled>{placeholder}</option>
    {/if}
    {#each options as option (option.value)}
      <option value={option.value} disabled={option.disabled}>{option.label}</option>
    {/each}
  </select>
  <span class="p-select__chevron"><ChevronDown size={14} strokeWidth={2} aria-hidden="true" /></span>
</div>

<style>
  .p-select {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }
  select {
    appearance: none;
    box-sizing: border-box;
    width: 100%;
    padding: 0 34px 0 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
    cursor: pointer;

    &:hover:not(:disabled) {
      border-color: #b8b8b8;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -1px;
      border-color: var(--p-focus);
    }
    &:disabled {
      background: var(--p-surface-muted);
      color: var(--p-text-secondary);
      cursor: not-allowed;
    }
  }
  .p-select--md select {
    height: 40px;
  }
  .p-select--sm select {
    height: 32px;
    font-size: 0.8125rem;
    padding-left: 10px;
  }
  .p-select--invalid select {
    border-color: var(--p-danger);
  }
  .p-select__chevron {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    color: var(--p-text-secondary);
    pointer-events: none;
  }
</style>
