<script lang="ts" module>
  import Calendar from "@lucide/svelte/icons/calendar";
  import Megaphone from "@lucide/svelte/icons/megaphone";
  import Rocket from "@lucide/svelte/icons/rocket";
  import Trophy from "@lucide/svelte/icons/trophy";
  import type { AnnouncementKind } from "../../api/types.ts";
  import type { IconComponent } from "../ui/types.ts";

  // The same icon for each kind as the home page ticker.
  const KIND_ICONS: Readonly<Record<AnnouncementKind, IconComponent>> = {
    event: Calendar,
    project_update: Rocket,
    milestone: Trophy,
    club_news: Megaphone,
  };
</script>

<script lang="ts">
  // An announcement at real size after the dog mark, as the home page ticker shows it (spec AP-2).
  // It mirrors the ticker's item; once the ticker components are merged this can use them.
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import type { AnnouncementInput } from "../../api/types.ts";
  import CommitteeChip from "../ui/CommitteeChip.svelte";
  import DogMark from "../ui/DogMark.svelte";

  let { announcement }: { announcement: AnnouncementInput } = $props();

  const Icon = $derived(KIND_ICONS[announcement.kind]);
</script>

<div class="announcement-preview">
  <p class="p-sr-only">As it will appear on the ticker: {announcement.message}</p>
  <!-- The facts under the preview give the type and committee in words. -->
  <div class="announcement-preview__run" aria-hidden="true">
    <DogMark variant="gradient" width={29} />
    <span class="announcement-preview__item">
      <Icon size={16} strokeWidth={1.75} />
      <span class="announcement-preview__message">{announcement.message}</span>
      <CommitteeChip committee={announcement.committee} />
      {#if announcement.linkUrl}<ArrowUpRight size={14} strokeWidth={2} />{/if}
    </span>
  </div>
</div>

<style>
  .announcement-preview {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    min-height: 68px;
    padding: 8px 16px;
    overflow: clip;
    border: 1px solid var(--p-border);
    border-radius: 12px;
    background: var(--p-surface);
  }
  .announcement-preview__run {
    display: flex;
    align-items: center;
    gap: 24px;
    min-width: 0;
  }
  .announcement-preview__run > :global(.dog-mark) {
    flex: none;
  }
  .announcement-preview__item {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    min-height: 36px;
    padding: 7px 12px;
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.9375rem;
    font-weight: 500;
    line-height: 22px;

    & > :global(svg) {
      flex: none;
    }
  }
  /* The ticker keeps it on one line; here a long message wraps so the approver reads all of it. */
  .announcement-preview__message {
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
