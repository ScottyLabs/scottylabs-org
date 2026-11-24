<script lang="ts">
  let {
    checked = $bindable(false),
    label,
    description,
    hideLabel = false,
    disabled = false,
    onchange,
    id,
    class: className,
  }: {
    checked?: boolean;
    label: string;
    description?: string;
    hideLabel?: boolean;
    disabled?: boolean;
    /** Called with the new value after the person flips it. */
    onchange?: (checked: boolean) => void;
    id?: string;
    class?: string;
  } = $props();

  const uid = $props.id();
  const buttonId = $derived(id ?? `switch-${uid}`);
  const labelId = $derived(`${buttonId}-label`);
  const descriptionId = $derived(`${buttonId}-description`);

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<div class={["p-switch", disabled && "p-switch--disabled", className]}>
  <button
    id={buttonId}
    type="button"
    role="switch"
    class="p-switch__track"
    aria-checked={checked ? "true" : "false"}
    aria-labelledby={labelId}
    aria-describedby={description ? descriptionId : undefined}
    {disabled}
    onclick={toggle}
  >
    <span class="p-switch__thumb"></span>
  </button>
  <span class={["p-switch__text", hideLabel && "p-sr-only"]}>
    <label id={labelId} class="p-switch__label" for={buttonId}>{label}</label>
    {#if description}
      <span id={descriptionId} class="p-switch__description">{description}</span>
    {/if}
  </span>
</div>

<style>
  .p-switch {
    display: inline-flex;
    align-items: flex-start;
    gap: 10px;
    font-family: var(--p-font);
  }
  .p-switch__track {
    box-sizing: border-box;
    position: relative;
    flex: none;
    width: 36px;
    height: 20px;
    margin-top: 0;
    border-radius: var(--p-radius-pill);
    background: #8c8c8c;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &[aria-checked="true"] {
      background: var(--p-inverse);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      background: var(--p-border);
      cursor: not-allowed;
    }
  }
  .p-switch__thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform var(--p-duration) var(--p-ease);
  }
  [aria-checked="true"] > .p-switch__thumb {
    transform: translateX(16px);
  }
  .p-switch__text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .p-switch__label {
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
    cursor: pointer;
  }
  .p-switch--disabled .p-switch__label {
    color: var(--p-text-secondary);
    cursor: not-allowed;
  }
  .p-switch__description {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
</style>
