<script lang="ts">
  // The announcement at real size between two dog marks (spec TK-6), as the ticker will show it.
  import type { PublicAnnouncement } from "../../api/types.ts";
  import DogMark from "../../portal/ui/DogMark.svelte";
  import TickerItem from "../TickerItem.svelte";

  let {
    announcement,
    isLive = false,
  }: {
    announcement: Pick<PublicAnnouncement, "kind" | "committee" | "message" | "linkUrl">;
    /** Editing one that is already live: it keeps its place. */
    isLive?: boolean;
  } = $props();
</script>

<section class="preview" aria-labelledby="composer-preview-heading">
  <header class="preview__head">
    <h2 id="composer-preview-heading" class="preview__title">Preview</h2>
    <span class="preview__size" aria-hidden="true">actual size</span>
  </header>
  <!-- The form above already says everything the preview shows, so it is hidden from screen readers. -->
  <div class="preview__strip" aria-hidden="true" inert>
    <DogMark variant="gradient" width={29} />
    <TickerItem {announcement} focusable={false} />
    <DogMark variant="gradient" width={29} />
  </div>
  <p class="preview__note">
    Bark sits between every announcement. {isLive ? "Yours keeps its place in the loop." : "Yours joins the end of the loop."}
  </p>
</section>

<style>
  .preview {
    overflow: clip;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .preview__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 44px;
    padding: 0 20px;
    border-bottom: 1px solid var(--p-border);
  }
  .preview__title {
    margin: 0;
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .preview__size {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .preview__strip {
    display: flex;
    align-items: center;
    gap: 28px;
    height: 66px;
    padding: 0 14px;
    overflow: clip;
    border-bottom: 1px solid var(--p-border);
  }
  .preview__note {
    margin: 0;
    padding: 12px 20px;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  /* Phones: the strip alone, edge to edge under the app bar, as the home page shows it. */
  @media (max-width: 600px) {
    .preview {
      margin-inline: -16px;
      border-width: 0 0 1px;
      border-radius: 0;
    }
    .preview__head,
    .preview__note {
      display: none;
    }
    .preview__strip {
      gap: 24px;
      height: 68px;
      border-bottom: 0;
    }
  }
</style>
