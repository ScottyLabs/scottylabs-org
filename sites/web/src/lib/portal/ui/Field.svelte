<script lang="ts">
  import type { Snippet } from "svelte";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import { setFieldContext } from "./field-context.ts";

  let {
    label,
    help,
    error,
    required = false,
    optional = false,
    counter,
    hideLabel = false,
    id: idProp,
    class: className,
    children,
  }: {
    label: string;
    /** A sentence under the input that explains what to enter. */
    help?: string;
    /** What is wrong and how to fix it. Announced when it changes. */
    error?: string | null;
    required?: boolean;
    /** Adds "(optional)" after the label. */
    optional?: boolean;
    /** Shows "12 / 90" and turns red past the limit. */
    counter?: { length: number; max: number };
    /** Keeps the label for assistive technology only. */
    hideLabel?: boolean;
    id?: string;
    class?: string;
    children: Snippet;
  } = $props();

  const uid = $props.id();
  const id = $derived(idProp ?? `field-${uid}`);
  const helpId = $derived(`${id}-help`);
  const counterId = $derived(`${id}-counter`);
  const errorId = $derived(`${id}-error`);
  const invalid = $derived(error !== null && error !== undefined && error !== "");
  const over = $derived(counter !== undefined && counter.length > counter.max);

  const describedBy = $derived(
    [help ? helpId : null, counter ? counterId : null, invalid ? errorId : null]
      .filter((value) => value !== null)
      .join(" ") || undefined,
  );

  setFieldContext({
    get id() {
      return id;
    },
    get describedBy() {
      return describedBy;
    },
    get invalid() {
      return invalid;
    },
    get required() {
      return required;
    },
  });
</script>

<div class={["p-field", className]}>
  <div class="p-field__top">
    <label class={["p-field__label", hideLabel && "p-sr-only"]} for={id}>
      {label}{#if optional}{" "}<span class="p-field__optional">(optional)</span>{/if}
    </label>
    {#if counter}
      <span id={counterId} class={["p-field__counter", over && "p-field__counter--over"]}>
        <span aria-hidden="true">{counter.length} / {counter.max}</span>
        <span class="p-sr-only">{counter.length} of {counter.max} characters</span>
      </span>
    {/if}
  </div>
  {@render children()}
  {#if help}
    <p id={helpId} class="p-field__help">{help}</p>
  {/if}
  <div id={errorId} class="p-field__error" aria-live="polite">
    {#if invalid}
      <CircleAlert size={14} strokeWidth={2} aria-hidden="true" />
      <span>{error}</span>
    {/if}
  </div>
</div>

<style>
  .p-field {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    font-family: var(--p-font);
  }
  .p-field__top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .p-field__label {
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .p-field__optional {
    color: var(--p-text-tertiary);
    font-weight: 400;
  }
  .p-field__counter {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
  }
  .p-field__counter--over {
    color: var(--p-danger-text);
  }
  .p-field__help {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .p-field__error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;

    &:empty {
      margin-top: -8px;
    }
    & :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
</style>
