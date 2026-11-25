<script lang="ts">
  // The live preview beside the editor (spec PR-4): the draft as a card on the Projects page, or
  // as its page at phone width. It is a picture of the result, so it takes no clicks or focus.
  import type { ProjectCard as Card, ProjectDetail } from "../../api/types.ts";
  import { SegmentedControl } from "../../portal/ui/index.ts";
  import ProjectCard from "../ProjectCard.svelte";
  import ProjectPage from "../ProjectPage.svelte";

  let { card, detail }: { card: Card; detail: ProjectDetail } = $props();

  let mode = $state<"card" | "page">("card");
  const MODES = [
    { value: "card", label: "Card" },
    { value: "page", label: "Page" },
  ] as const;
</script>

<section class="preview" aria-labelledby="preview-heading">
  <div class="preview__head">
    <h2 class="preview__heading" id="preview-heading">Live preview</h2>
    <SegmentedControl label="Preview as" options={MODES} bind:value={mode} size="sm" />
  </div>
  <p class="p-sr-only">
    The preview shows the draft as visitors would see it. Everything in it is also in the form.
  </p>
  {#if mode === "card"}
    <div class="preview__frame">
      <div class="preview__card" inert aria-hidden="true">
        <ProjectCard project={card} href={null} headingLevel={3} loading="eager" />
      </div>
    </div>
  {:else}
    <!-- The frame scrolls, so it takes focus for keyboard scrolling; its content is a picture. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="preview__frame preview__frame--page"
      role="region"
      aria-label="Page preview. Scroll to see all of it."
      tabindex="0"
    >
      <div class="preview__page" inert aria-hidden="true">
        <ProjectPage project={detail} />
      </div>
    </div>
  {/if}
</section>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: 16px;
    color: var(--p-text);
  }
  .preview__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .preview__heading {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .preview__frame {
    padding: 20px;
    border-radius: 20px;
    background: var(--p-surface-muted);
  }
  .preview__card {
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .preview__frame--page {
    max-height: min(640px, 70vh);
    overflow: auto;
    padding: 0;
    border: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .preview__frame:focus-visible {
    outline: 2px solid var(--p-focus);
    outline-offset: 2px;
  }
  .preview__page {
    padding: 16px;
    font-size: 0.9375rem;
  }
</style>
