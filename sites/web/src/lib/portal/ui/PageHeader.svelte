<script lang="ts">
  // The top of a portal page: the h1 (focused after navigation), a line under it and actions.
  import type { Snippet } from "svelte";

  let {
    title,
    description,
    eyebrow,
    actions,
    class: className,
  }: {
    title: string;
    description?: string;
    /** Small text above the title, such as the date and role. */
    eyebrow?: Snippet;
    actions?: Snippet;
    class?: string;
  } = $props();
</script>

<header class={["p-page-header", className]}>
  <div class="p-page-header__text">
    {#if eyebrow}<div class="p-page-header__eyebrow">{@render eyebrow()}</div>{/if}
    <h1 class="p-page-header__title" tabindex="-1">{title}</h1>
    {#if description}<p class="p-page-header__description">{description}</p>{/if}
  </div>
  {#if actions}<div class="p-page-header__actions">{@render actions()}</div>{/if}
</header>

<style>
  .p-page-header {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px 24px;
    font-family: var(--p-font);
  }
  .p-page-header__text {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .p-page-header__eyebrow {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    margin-bottom: 2px;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .p-page-header__title {
    margin: 0;
    color: var(--p-text);
    font-size: 2.75rem;
    font-weight: 700;
    line-height: 1.14;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;

    &:focus {
      outline: none;
    }
    @media (max-width: 600px) {
      font-size: 2.125rem;
    }
  }
  .p-page-header__description {
    max-width: 62ch;
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .p-page-header__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
