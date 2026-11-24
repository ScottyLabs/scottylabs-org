<script lang="ts">
  import type { StatusTone } from "./types.ts";

  let {
    tone = "neutral",
    label,
    size = "md",
    class: className,
  }: {
    tone?: StatusTone;
    /** "Published", "Unpublished changes", "Draft". Omit for a dot alone. */
    label?: string;
    size?: "sm" | "md";
    class?: string;
  } = $props();
</script>

<span class={["p-status", `p-status--${tone}`, `p-status--${size}`, className]}>
  <span class="p-status__dot" aria-hidden="true"></span>
  {#if label !== undefined}<span class="p-status__label">{label}</span>{/if}
</span>

<style>
  .p-status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-family: var(--p-font);
    color: var(--p-text);
    white-space: nowrap;
  }
  .p-status--md {
    font-size: 0.875rem;
    line-height: 20px;
  }
  .p-status--sm {
    gap: 6px;
    font-size: 0.75rem;
    line-height: 16px;
  }
  .p-status__dot {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    background: var(--dot);
  }
  .p-status--sm .p-status__dot {
    width: 6px;
    height: 6px;
  }
  .p-status--success {
    --dot: var(--p-success);
  }
  .p-status--warning {
    --dot: var(--p-warning);
    color: var(--p-warning-text);
  }
  .p-status--info {
    --dot: var(--p-info);
  }
  .p-status--neutral {
    --dot: var(--p-neutral);
  }
  .p-status--danger {
    --dot: var(--p-danger);
    color: var(--p-danger-text);
  }
</style>
