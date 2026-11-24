<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    tone = "neutral",
    children,
    class: className,
  }: {
    tone?: "neutral" | "info" | "warning" | "success" | "danger";
    children: Snippet;
    class?: string;
  } = $props();
</script>

<span class={["p-badge", `p-badge--${tone}`, className]}>{@render children()}</span>

<style>
  .p-badge {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    height: 20px;
    padding: 0 8px;
    border-radius: var(--p-radius-pill);
    font-family: var(--p-font);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 14px;
    white-space: nowrap;
  }
  .p-badge--neutral {
    background: var(--p-surface-strong);
    color: var(--p-text);
  }
  .p-badge--info {
    background: var(--p-info-surface);
    color: var(--p-info-text);
  }
  .p-badge--warning {
    background: var(--p-warning-surface);
    color: var(--p-warning-text);
  }
  .p-badge--success {
    background: #eaf6ed;
    color: var(--p-success);
  }
  .p-badge--danger {
    background: var(--p-danger-surface);
    color: var(--p-danger-text);
  }
</style>
