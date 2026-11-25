<script lang="ts">
  // "Recent activity" (spec O-5): the last changes in the viewer's scope, newest first. Entries
  // link to the page they are about when the viewer can open it.
  import type { ActivityEntry } from "../../api/types.ts";
  import { Avatar } from "../ui/index.ts";
  import { activityText, activityTime } from "./copy.ts";

  let {
    entries,
    viewerId,
    now,
  }: { entries: readonly ActivityEntry[]; viewerId: string; now: Date } = $props();

  const uid = $props.id();
</script>

<section class="activity" aria-labelledby={`activity-${uid}`}>
  <h2 id={`activity-${uid}`} class="activity__title">Recent activity</h2>
  {#if entries.length === 0}
    <p class="activity__empty">Nothing has changed yet. Edits, approvals and role changes show up here.</p>
  {:else}
    <ol class="activity__list">
      {#each entries as entry (entry.id)}
        <li class="activity__entry">
          <Avatar initials={entry.actor?.initials ?? "SL"} size="sm" />
          <div class="activity__text">
            {#if entry.href !== null}
              <a class="activity__summary activity__summary--link" href={entry.href}>{activityText(entry, viewerId)}</a>
            {:else}
              <p class="activity__summary">{activityText(entry, viewerId)}</p>
            {/if}
            <time class="activity__time" datetime={entry.at}>{activityTime(entry.at, now)}</time>
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .activity {
    box-sizing: border-box;
    min-width: 0;
    padding-bottom: 6px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .activity__title {
    margin: 0;
    padding: 16px 20px 6px;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .activity__empty {
    margin: 0;
    padding: 8px 20px 14px;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .activity__list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .activity__entry {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 20px;
  }
  .activity__text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .activity__summary {
    margin: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
    overflow-wrap: anywhere;
  }
  .activity__summary--link {
    text-decoration: none;
    border-radius: 4px;

    &:hover {
      text-decoration: underline;
      text-underline-offset: 2px;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .activity__time {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
</style>
