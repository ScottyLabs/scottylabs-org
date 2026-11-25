<script lang="ts">
  // When and where the committee meets, from the page settings, with a link to CMU Maps.
  import type { CommitteeSettings } from "../../api/types.ts";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import { headingTag } from "./types.ts";

  let { settings, editor }: { settings: CommitteeSettings; editor: boolean } = $props();
</script>

<section class="sessions">
  <svelte:element this={headingTag(2, editor)} class="sessions__label">Work sessions</svelte:element>
  <p class="sessions__time">
    {#if settings.meetingTime !== ""}{settings.meetingTime}{:else if editor}<span class="cb-placeholder">Set the meeting time in the page settings</span>{/if}
  </p>
  {#if settings.meetingRoom !== ""}
    <p class="sessions__room">
      <MapPin size={18} strokeWidth={2} aria-hidden="true" />
      <span><span class="cb-sr-only">Room: </span>{settings.meetingRoom}</span>
    </p>
  {/if}
  <p class="sessions__note">Come by any week and meet the team.</p>
  {#if settings.mapUrl}
    {#if editor}
      <span class="cb-btn cb-btn--outline cb-btn--sm sessions__map">Open in CMU Maps</span>
    {:else}
      <a class="cb-btn cb-btn--outline cb-btn--sm sessions__map" href={settings.mapUrl} target="_blank" rel="noopener noreferrer">
        Open in CMU Maps<span class="cb-sr-only"> (opens in a new tab)</span>
      </a>
    {/if}
  {/if}
</section>

<style>
  .sessions {
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-sizing: border-box;
    padding: 28px;
    border-radius: 16px;
    background: var(--cb-tint);
  }
  .sessions__label {
    margin: 0;
    color: var(--cb-on-tint);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .sessions__time {
    margin: 0;
    color: var(--cb-text);
    font-size: 1.5rem;
    font-weight: 500;
    line-height: 32px;
  }
  .sessions__room {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    color: var(--cb-text);
    font-size: 1rem;
    line-height: 24px;

    & > :global(svg) {
      flex: none;
      margin-top: 3px;
    }
  }
  .sessions__note {
    margin: 0;
    color: var(--cb-on-tint);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .sessions__map {
    align-self: flex-start;
    margin-top: 8px;
    background: transparent;
    border-color: rgba(0, 0, 0, 0.16);
  }

  @container block (width < 300px) {
    .sessions {
      padding: 20px;
    }
    .sessions__time {
      font-size: 1.25rem;
      line-height: 28px;
    }
  }
</style>
