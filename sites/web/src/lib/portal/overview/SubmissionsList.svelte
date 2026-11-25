<script lang="ts">
  // A lead's own submissions and where each stands (spec AP-5): waiting, approved, or sent back
  // with the director's note. Sent back ones link to the editor so the lead can fix and resend.
  import type { Approval } from "../../api/types.ts";
  import { submissionStatus, submissionSubject } from "../approvals/format.ts";
  import { Button, StatusDot } from "../ui/index.ts";

  let { submissions, now }: { submissions: readonly Approval[]; now: Date } = $props();

  const uid = $props.id();
</script>

<section class="submissions" aria-labelledby={`submissions-${uid}`}>
  <h2 id={`submissions-${uid}`} class="submissions__title">Your submissions</h2>
  <ul class="submissions__list">
    {#each submissions as approval (approval.id)}
      {@const status = submissionStatus(approval, now)}
      {@const subjectId = `submission-${uid}-${approval.id}`}
      <li class="submissions__item">
        <div class="submissions__text">
          <p id={subjectId} class="submissions__subject">{submissionSubject(approval)}</p>
          <p class="submissions__status">
            <StatusDot tone={status.tone} label={status.label} size="sm" />
            <span class="submissions__detail">{status.detail}</span>
          </p>
          {#if approval.note}
            <p class={["submissions__note", approval.status === "sent_back" && "submissions__note--change"]}>
              Note: {approval.note}
            </p>
          {/if}
        </div>
        {#if approval.status === "sent_back"}
          <Button size="sm" variant="outline" href={approval.href} aria-describedby={subjectId}>Edit</Button>
        {/if}
      </li>
    {/each}
  </ul>
</section>

<style>
  .submissions {
    box-sizing: border-box;
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .submissions__title {
    margin: 0;
    padding: 16px 20px 14px;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .submissions__list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .submissions__item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 20px;
    border-top: 1px solid var(--p-border);
  }
  .submissions__text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .submissions__subject,
  .submissions__status,
  .submissions__note {
    margin: 0;
    font-size: 0.75rem;
    line-height: 16px;
    overflow-wrap: anywhere;
  }
  .submissions__subject {
    font-weight: 500;
  }
  .submissions__status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
  }
  .submissions__detail {
    color: var(--p-text-tertiary);
  }
  .submissions__note {
    padding: 6px 10px;
    border-radius: var(--p-radius-sm);
    background: var(--p-surface-muted);
    color: var(--p-text);
  }
  .submissions__note--change {
    background: var(--p-warning-surface);
  }
</style>
