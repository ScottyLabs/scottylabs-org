<script lang="ts">
  import type { Snippet } from "svelte";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import CircleCheck from "@lucide/svelte/icons/circle-check";
  import Info from "@lucide/svelte/icons/info";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";

  let {
    tone = "info",
    title,
    role,
    children,
    actions,
    class: className,
  }: {
    tone?: "info" | "success" | "warning" | "error";
    title?: string;
    /** "alert" announces it at once; use it for errors that appear after an action. */
    role?: "alert" | "status";
    children?: Snippet;
    actions?: Snippet;
    class?: string;
  } = $props();

  const icons = { info: Info, success: CircleCheck, warning: TriangleAlert, error: CircleAlert } as const;
  const Icon = $derived(icons[tone]);
</script>

<div class={["p-callout", `p-callout--${tone}`, className]} {role}>
  <span class="p-callout__icon"><Icon size={16} strokeWidth={2} aria-hidden="true" /></span>
  <div class="p-callout__content">
    {#if title}<p class="p-callout__title">{title}</p>{/if}
    {#if children}<div class="p-callout__body">{@render children()}</div>{/if}
    {#if actions}<div class="p-callout__actions">{@render actions()}</div>{/if}
  </div>
</div>

<style>
  .p-callout {
    box-sizing: border-box;
    display: flex;
    gap: 10px;
    padding: 12px 14px;
    border: 1px solid var(--callout-border);
    border-radius: var(--p-radius);
    background: var(--callout-bg);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .p-callout__icon {
    display: grid;
    flex: none;
    height: 20px;
    place-items: center;
    color: var(--callout-icon);
  }
  .p-callout__content {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .p-callout__title {
    margin: 0;
    font-size: 0.875rem;
    font-weight: 700;
    line-height: 20px;
  }
  .p-callout__body {
    color: var(--p-text);
    & :global(p) {
      margin: 0;
      font-size: inherit;
    }
    & :global(a) {
      color: inherit;
      text-decoration: underline;
    }
  }
  .p-callout__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .p-callout--info {
    --callout-bg: var(--p-info-surface);
    --callout-border: #bcdcf7;
    --callout-icon: var(--p-info-text);
  }
  .p-callout--success {
    --callout-bg: #eaf6ed;
    --callout-border: #b9dfc2;
    --callout-icon: var(--p-success);
  }
  .p-callout--warning {
    --callout-bg: var(--p-warning-surface);
    --callout-border: #f3d4a8;
    --callout-icon: var(--p-warning-text);
  }
  .p-callout--error {
    --callout-bg: var(--p-danger-surface);
    --callout-border: #f3c0c0;
    --callout-icon: var(--p-danger-text);
  }
</style>
