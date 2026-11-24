<script lang="ts">
  import type { HTMLTextareaAttributes } from "svelte/elements";
  import { getFieldContext } from "./field-context.ts";

  type Props = Omit<HTMLTextareaAttributes, "value" | "children"> & {
    value?: string;
    invalid?: boolean;
    ref?: HTMLTextAreaElement | null;
  };

  let {
    value = $bindable(""),
    ref = $bindable(null),
    invalid,
    rows = 4,
    id,
    class: className,
    ...rest
  }: Props = $props();

  const field = getFieldContext();
  const isInvalid = $derived(invalid ?? field?.invalid ?? false);
</script>

<textarea
  bind:this={ref}
  bind:value
  {...rest}
  {rows}
  class={["p-textarea", isInvalid && "p-textarea--invalid", className]}
  id={id ?? field?.id}
  aria-describedby={rest["aria-describedby"] ?? field?.describedBy}
  aria-invalid={isInvalid ? "true" : undefined}
  required={rest.required ?? field?.required}
></textarea>

<style>
  .p-textarea {
    box-sizing: border-box;
    width: 100%;
    min-height: 80px;
    padding: 9px 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
    resize: vertical;

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
  }
  .p-textarea--invalid {
    border-color: var(--p-danger);
  }
</style>
