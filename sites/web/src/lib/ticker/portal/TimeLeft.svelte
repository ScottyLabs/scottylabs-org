<script lang="ts">
  // "in 6 days  Mon, Nov 30, 11:59 PM" over a bar of the time left, amber within three days.
  import type { Announcement } from "../../api/types.ts";
  import { formatAbsolute } from "../../portal/dates.ts";
  import { timeLeft } from "../format.ts";

  let { announcement, now }: { announcement: Announcement; now: Date } = $props();

  const left = $derived(timeLeft(announcement, now));
</script>

<span class={["time-left", left.soon && "time-left--soon"]}>
  <span class="time-left__text">
    <span class="time-left__relative">{left.label}</span>
    <span class="time-left__absolute">{formatAbsolute(announcement.expiresAt, now)}</span>
  </span>
  <span class="time-left__bar" aria-hidden="true">
    <span class="time-left__fill" style:width="{Math.round(left.fraction * 100)}%"></span>
  </span>
</span>

<style>
  .time-left {
    display: inline-flex;
    flex-direction: column;
    gap: 6px;
    min-width: 160px;
  }
  .time-left__text {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 6px;
  }
  .time-left__relative {
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
    white-space: nowrap;
  }
  .time-left__absolute {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .time-left--soon .time-left__relative {
    color: var(--p-warning-text);
  }
  .time-left__bar {
    display: block;
    width: 160px;
    height: 3px;
    border-radius: 3px;
    background: var(--p-surface-strong);
    overflow: hidden;
  }
  .time-left__fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: var(--p-text);
  }
  .time-left--soon .time-left__fill {
    background: var(--p-warning);
  }
</style>
