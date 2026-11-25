<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";
  import { getFieldContext } from "./field-context.ts";
  import type { IconComponent } from "./types.ts";

  type Props = Omit<HTMLInputAttributes, "value" | "children"> & {
    value?: string;
    /** Shown inside the input on the left, such as a search icon. */
    icon?: IconComponent;
    invalid?: boolean;
    /** The input element, for focusing it from outside. */
    ref?: HTMLInputElement | null;
  };

  let {
    value = $bindable(""),
    ref = $bindable(null),
    icon: Icon,
    invalid,
    type = "text",
    id,
    class: className,
    ...rest
  }: Props = $props();

  const field = getFieldContext();
  const isInvalid = $derived(invalid ?? field?.invalid ?? false);
</script>

<div class={["p-input", Icon && "p-input--icon", isInvalid && "p-input--invalid", className]}>
  {#if Icon}
    <span class="p-input__icon"><Icon size={16} strokeWidth={1.75} aria-hidden="true" /></span>
  {/if}
  <input
    bind:this={ref}
    bind:value
    {...rest}
    {type}
    id={id ?? field?.id}
    aria-describedby={rest["aria-describedby"] ?? field?.describedBy}
    aria-invalid={isInvalid ? "true" : undefined}
    required={rest.required ?? field?.required}
  />
</div>

<style>
  .p-input {
    position: relative;
    display: flex;
    min-width: 0;
  }
  .p-input input {
    box-sizing: border-box;
    width: 100%;
    min-height: var(--p-field-height);
    padding: calc((var(--p-field-height) - var(--p-field-line) - 2px) / 2) 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font: inherit;
    font-family: var(--p-font);
    font-size: var(--p-field-font);
    line-height: var(--p-field-line);
    transition: border-color var(--p-duration) var(--p-ease);

    &::placeholder {
      color: var(--p-placeholder);
    }
    &:hover:not(:disabled):not(:read-only) {
      border-color: #b8b8b8;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -1px;
      border-color: var(--p-focus);
    }
    &:disabled,
    &:read-only {
      background: var(--p-surface-muted);
      color: var(--p-text-secondary);
    }
    &:disabled {
      cursor: not-allowed;
    }
  }
  .p-input--icon input {
    padding-left: 36px;
  }
  .p-input__icon {
    position: absolute;
    left: 12px;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    color: var(--p-text-secondary);
    pointer-events: none;
  }
  .p-input--invalid input {
    border-color: var(--p-danger);
  }
</style>
