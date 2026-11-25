<script lang="ts">
  // "Needs attention" (spec O-4): each item with a colored dot, what it is, where it is, and one
  // action. Waiting approvals come first and their Review button is the filled one.
  import type { AttentionItem } from "../../api/types.ts";
  import { Button } from "../ui/index.ts";

  let {
    items,
    heading = $bindable(null),
  }: {
    items: readonly AttentionItem[];
    /** The heading, so the page can move focus here after a decision removes an item. */
    heading?: HTMLHeadingElement | null;
  } = $props();

  const uid = $props.id();

  /** Links that only add `?review=` keep focus and scroll, so the dialog can hand focus back. */
  function staysOnPage(href: string): boolean {
    return href.startsWith("/portal/?");
  }
</script>

<section class="attention" aria-labelledby={`attention-${uid}`}>
  <header class="attention__head">
    <h2 id={`attention-${uid}`} class="attention__title" tabindex="-1" bind:this={heading}>
      Needs attention
    </h2>
    <span class="attention__count">
      <span aria-hidden="true">{items.length}</span>
      <span class="p-sr-only">{items.length === 1 ? "1 item" : `${items.length} items`}</span>
    </span>
  </header>
  {#if items.length === 0}
    <p class="attention__empty">Nothing needs you right now. New approvals and reminders show up here.</p>
  {:else}
    <ul class="attention__list">
      {#each items as item, index (`${item.kind}-${item.action.href}-${index}`)}
        {@const titleId = `attention-${uid}-${index}`}
        <li class="attention__item">
          <span class={["attention__dot", `attention__dot--${item.severity}`]} aria-hidden="true"></span>
          <div class="attention__text">
            <p id={titleId} class="attention__item-title">{item.title}</p>
            <p class="attention__detail">{item.detail}</p>
          </div>
          <Button
            class="attention__action"
            size="sm"
            variant={item.kind === "approval" ? "filled" : "outline"}
            href={item.action.href}
            aria-describedby={titleId}
            data-sveltekit-reset={staysOnPage(item.action.href) ? "false" : undefined}
          >
            {item.action.label}
          </Button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .attention {
    box-sizing: border-box;
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .attention__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 20px 14px;
  }
  .attention__title {
    margin: 0;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;

    &:focus {
      outline: none;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .attention__count {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .attention__empty {
    margin: 0;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--p-border);
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .attention__list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .attention__item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 20px;
    border-top: 1px solid var(--p-border);
  }
  .attention__dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .attention__dot--info {
    background: var(--p-info);
  }
  .attention__dot--warning {
    background: var(--p-warning);
  }
  .attention__dot--neutral {
    background: var(--p-neutral);
  }
  .attention__text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .attention__item-title,
  .attention__detail {
    margin: 0;
    font-size: 0.75rem;
    line-height: 16px;
    overflow-wrap: anywhere;
  }
  .attention__item-title {
    font-weight: 500;
  }
  .attention__detail {
    color: var(--p-text-tertiary);
  }
  .attention :global(.attention__action) {
    flex: none;
  }
  @media (max-width: 899px) {
    .attention__head {
      padding: 14px 16px 12px;
    }
    .attention__item {
      gap: 12px;
      padding: 12px 16px;
    }
    .attention__dot {
      align-self: flex-start;
      margin-top: 4px;
    }
  }
</style>
