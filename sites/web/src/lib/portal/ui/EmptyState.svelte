<script lang="ts">
  import type { Snippet } from "svelte";
  import type { IconComponent } from "./types.ts";

  let {
    icon: Icon,
    title,
    description,
    headingLevel = 2,
    actions,
    class: className,
  }: {
    icon?: IconComponent;
    title: string;
    /** Says what to do next. */
    description?: string;
    headingLevel?: 2 | 3 | 4;
    actions?: Snippet;
    class?: string;
  } = $props();
</script>

<div class={["p-empty", className]}>
  {#if Icon}
    <span class="p-empty__icon"><Icon size={20} strokeWidth={1.75} aria-hidden="true" /></span>
  {/if}
  <svelte:element this={`h${headingLevel}`} class="p-empty__title">{title}</svelte:element>
  {#if description}<p class="p-empty__description">{description}</p>{/if}
  {#if actions}<div class="p-empty__actions">{@render actions()}</div>{/if}
</div>

<style>
  .p-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px 24px;
    text-align: center;
    font-family: var(--p-font);
  }
  .p-empty__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-bottom: 6px;
    border-radius: 12px;
    background: var(--p-surface-muted);
    color: var(--p-text);
  }
  .p-empty__title {
    margin: 0;
    color: var(--p-text);
    font-size: 1rem;
    font-weight: 500;
    line-height: 24px;
  }
  .p-empty__description {
    max-width: 44ch;
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .p-empty__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 10px;
  }
</style>
