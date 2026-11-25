<script lang="ts">
  // "Before you publish" (Figma 109:6800; spec PR-5): what the draft still needs, from the
  // server's checklist, and who has to approve it.
  import CheckCheck from "@lucide/svelte/icons/check-check";
  import CircleDashed from "@lucide/svelte/icons/circle-dashed";
  import Info from "@lucide/svelte/icons/info";
  import type { PersonRef, ProjectChecklist } from "../../api/types.ts";
  import { MIN_COVER_WIDTH } from "../../media/images.ts";
  import { MAX_SUMMARY_CHARS } from "../drafts.ts";

  let {
    checklist,
    needsApproval,
    approvers,
  }: {
    checklist: ProjectChecklist;
    needsApproval: boolean;
    approvers: readonly PersonRef[];
  } = $props();

  const items = $derived([
    { done: checklist.cover, required: true, label: "Cover photo added" },
    // The service counts no cover as wide enough; the item is done only once a wide one is there.
    {
      done: checklist.cover && checklist.coverWide,
      required: true,
      label: `Cover photo at least ${MIN_COVER_WIDTH.toLocaleString("en-US")} pixels wide`,
    },
    { done: checklist.altText, required: true, label: "Cover photo described" },
    {
      done: checklist.summary,
      required: true,
      label: `Summary of ${MAX_SUMMARY_CHARS.toString()} characters or fewer`,
    },
    { done: checklist.description, required: false, label: "Description written" },
    { done: checklist.links, required: false, label: "Live site or repository linked" },
  ]);
  const approval = $derived.by(() => {
    if (!needsApproval) return "No approval needed for directors and admins";
    if (approvers.length === 0) return "A director or admin approves it before it goes live";
    return `${approvers.map((person) => person.name).join(" or ")} approves it before it goes live`;
  });
</script>

<section class="checklist" aria-labelledby="checklist-heading">
  <h2 class="checklist__heading" id="checklist-heading">Before you publish</h2>
  <ul class="checklist__items" role="list">
    {#each items as item (item.label)}
      <li class={["checklist__item", item.done && "checklist__item--done"]}>
        {#if item.done}
          <CheckCheck size={18} strokeWidth={2} aria-hidden="true" />
        {:else}
          <CircleDashed size={18} strokeWidth={1.75} aria-hidden="true" />
        {/if}
        <span>
          {item.label}
          <span class="p-sr-only">{item.done ? ": done" : ": not yet"}</span>
          {#if !item.done}
            <span class="checklist__tag">{item.required ? "Required" : "Recommended"}</span>
          {/if}
        </span>
      </li>
    {/each}
    <li class="checklist__item checklist__item--note">
      <Info size={18} strokeWidth={1.75} aria-hidden="true" />
      <span>{approval}</span>
    </li>
  </ul>
</section>

<style>
  .checklist {
    padding: 20px;
    border: 1px solid var(--p-border);
    border-radius: 20px;
    background: var(--p-surface);
    color: var(--p-text);
  }
  .checklist__heading {
    margin: 0 0 12px;
    font-size: 0.9375rem;
    font-weight: 500;
    line-height: 22px;
  }
  .checklist__items {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
  }
  .checklist__item {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;

    & :global(svg) {
      flex: none;
      margin-top: 1px;
    }
  }
  .checklist__item--done {
    color: var(--p-text);
  }
  .checklist__item--note {
    color: var(--p-text-secondary);
  }
  .checklist__tag {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--p-surface-muted);
    color: var(--p-text-secondary);
    font-size: 0.6875rem;
    line-height: 16px;
    white-space: nowrap;
  }
</style>
