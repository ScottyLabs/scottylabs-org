<script lang="ts">
  // An announcement at real size after the dog mark, drawn by the home page ticker's own item
  // (spec AP-2). The facts under it give the type, committee and link in words.
  import type { AnnouncementInput } from "../../api/types.ts";
  import TickerItem from "../../ticker/TickerItem.svelte";
  import DogMark from "../ui/DogMark.svelte";

  let { announcement }: { announcement: AnnouncementInput } = $props();
</script>

<div class="announcement-preview">
  <p class="p-sr-only">As it will appear on the ticker: {announcement.message}</p>
  <div class="announcement-preview__run" aria-hidden="true" inert>
    <DogMark variant="gradient" width={29} />
    <TickerItem class="announcement-preview__item" {announcement} focusable={false} />
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
  /* The ticker keeps an item on one line and scrolls it; here a long message wraps instead, so the
     approver reads all of it. */
  .announcement-preview__run > :global(.announcement-preview__item) {
    flex: 0 1 auto;
    min-width: 0;
    height: auto;
    min-height: 36px;
    white-space: normal;
  }
  .announcement-preview__run :global(.announcement-preview__item .ticker-item__message) {
    overflow: visible;
    overflow-wrap: anywhere;
  }
</style>
