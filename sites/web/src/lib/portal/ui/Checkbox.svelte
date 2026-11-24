<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import Check from "@lucide/svelte/icons/check";
  import Minus from "@lucide/svelte/icons/minus";

  type Props = Omit<HTMLInputAttributes, "type" | "checked" | "children"> & {
    checked?: boolean;
    indeterminate?: boolean;
    label: string;
    /** A second line under the label. */
    description?: string;
    /** Keeps the label for assistive technology only, for checkboxes in table rows. */
    hideLabel?: boolean;
  };

  let {
    checked = $bindable(false),
    indeterminate = $bindable(false),
    label,
    description,
    hideLabel = false,
    disabled = false,
    id,
    class: className,
    ...rest
  }: Props = $props();

  const uid = $props.id();
  const inputId = $derived(id ?? `check-${uid}`);
  const descriptionId = $derived(`${inputId}-description`);
</script>

<label class={["p-check", disabled && "p-check--disabled", className]} for={inputId}>
  <span class="p-check__control">
    <input
      {...rest}
      id={inputId}
      type="checkbox"
      bind:checked
      bind:indeterminate
      {disabled}
      aria-describedby={description ? descriptionId : undefined}
    />
    <span class="p-check__box" aria-hidden="true">
      {#if indeterminate}
        <Minus size={13} strokeWidth={3} />
      {:else if checked}
        <Check size={13} strokeWidth={3} />
      {/if}
    </span>
  </span>
  <span class={["p-check__text", hideLabel && "p-sr-only"]}>
    <span class="p-check__label">{label}</span>
    {#if description}
      <span id={descriptionId} class="p-check__description">{description}</span>
    {/if}
  </span>
</label>

<style>
  .p-check {
    display: inline-flex;
    align-items: flex-start;
    gap: 10px;
    font-family: var(--p-font);
    cursor: pointer;
  }
  .p-check--disabled {
    cursor: not-allowed;
  }
  .p-check__control {
    position: relative;
    display: grid;
    flex: none;
    width: 18px;
    height: 18px;
    margin-top: 1px;
  }
  input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: inherit;
  }
  .p-check__box {
    box-sizing: border-box;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 1.5px solid #8c8c8c;
    border-radius: 5px;
    background: var(--p-surface);
    color: var(--p-on-inverse);
    pointer-events: none;
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease);
  }
  input:checked + .p-check__box,
  input:indeterminate + .p-check__box {
    border-color: var(--p-inverse);
    background: var(--p-inverse);
  }
  input:focus-visible + .p-check__box {
    outline: 2px solid var(--p-focus);
    outline-offset: 2px;
  }
  input:disabled + .p-check__box {
    border-color: var(--p-border);
    background: var(--p-surface-muted);
    color: var(--p-text-disabled);
  }
  .p-check:hover input:not(:disabled):not(:checked) + .p-check__box {
    border-color: var(--p-text);
  }
  .p-check__text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .p-check__label {
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .p-check--disabled .p-check__label {
    color: var(--p-text-secondary);
  }
  .p-check__description {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
</style>
