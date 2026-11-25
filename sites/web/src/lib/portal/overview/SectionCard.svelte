<script lang="ts">
  // One section of the overview (Figma 106:4378): an icon and name with a small count, a picture
  // of what is live, a headline, a sentence and up to two actions. On phones (Figma 112:9558) it
  // keeps the headline and the main action only.
  import type { Snippet } from "svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import { Button } from "../ui/index.ts";
  import type { IconComponent } from "../ui/types.ts";
  import type { CardCopy } from "./copy.ts";

  let {
    icon: Icon,
    copy,
    /** The primary action adds something new, so it gets a plus. */
    primaryAdds = false,
    /** Why the primary action is turned off, shown under the actions. */
    blocked = null,
    visual,
  }: {
    icon: IconComponent;
    copy: CardCopy;
    primaryAdds?: boolean;
    blocked?: string | null;
    visual?: Snippet;
  } = $props();

  const uid = $props.id();
  const titleId = `overview-card-${uid}`;
  const blockedId = `overview-card-${uid}-blocked`;
</script>

<section class="overview-card" aria-labelledby={titleId}>
  <header class="overview-card__head">
    <span class="overview-card__icon"><Icon size={16} strokeWidth={1.75} aria-hidden="true" /></span>
    <h2 id={titleId} class="overview-card__title">{copy.title}</h2>
    <span class="overview-card__meta">{copy.meta}</span>
  </header>
  {#if visual}<div class="overview-card__visual">{@render visual()}</div>{/if}
  <p class="overview-card__headline">{copy.headline}</p>
  <p class="overview-card__text">{copy.text}</p>
  {#if copy.primary !== null || copy.secondary !== null}
    <div class="overview-card__actions">
      {#if copy.primary !== null}
        <Button
          class="overview-card__primary"
          size="sm"
          href={copy.primary.href}
          icon={primaryAdds ? Plus : undefined}
          disabled={blocked !== null}
          aria-describedby={blocked === null ? undefined : blockedId}
        >
          {copy.primary.label}
        </Button>
      {/if}
      {#if copy.secondary !== null}
        <Button class="overview-card__secondary" variant="subtle" size="sm" href={copy.secondary.href}>
          {copy.secondary.label}
        </Button>
      {/if}
    </div>
    {#if blocked !== null}<p id={blockedId} class="overview-card__blocked">{blocked}</p>{/if}
  {/if}
</section>

<style>
  .overview-card {
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    padding: 20px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .overview-card__head {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .overview-card__icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    background: var(--p-surface-muted);
  }
  .overview-card__title {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .overview-card__meta {
    flex: none;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .overview-card__visual {
    display: flex;
    align-items: center;
  }
  .overview-card__headline {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .overview-card__text {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .overview-card__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: auto;
    padding-top: 4px;
  }
  .overview-card__blocked {
    margin: -6px 0 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }

  @media (max-width: 899px) {
    .overview-card {
      gap: 12px;
      padding: 16px;
    }
    .overview-card__visual,
    .overview-card__text,
    .overview-card__actions :global(.overview-card__secondary) {
      display: none;
    }
    .overview-card__headline {
      font-size: 1rem;
      line-height: 22px;
    }
    .overview-card__actions :global(.overview-card__primary) {
      width: 100%;
      height: 44px;
      font-size: 0.9375rem;
      font-weight: 700;
    }
    .overview-card__actions :global(.overview-card__primary svg) {
      display: none;
    }
  }
</style>
