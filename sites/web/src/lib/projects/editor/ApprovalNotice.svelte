<script lang="ts">
  // Where a lead's submission stands (spec AP-5): waiting for a director, or sent back with a
  // note. Directors see that a lead's changes are waiting for them.
  import type { ApprovalRef, PersonRef } from "../../api/types.ts";
  import { formatRelative } from "../../portal/dates.ts";
  import { Button, Callout } from "../../portal/ui/index.ts";

  let {
    approval,
    approvers,
    isLead,
  }: {
    approval: ApprovalRef;
    approvers: readonly PersonRef[];
    /** True when the person editing is the lead who sends it, not an approver. */
    isLead: boolean;
  } = $props();

  const approverNames = $derived(
    approvers.length === 0 ? "a director" : approvers.map((person) => person.name).join(" or "),
  );
</script>

{#if approval.status === "waiting"}
  {#if isLead}
    <Callout tone="info" title={`Waiting for approval from ${approverNames}`} role="status">
      Visitors keep seeing the published version until it is approved. Any change you make now
      takes the request back, so send it again when you are done.
    </Callout>
  {:else}
    <Callout tone="info" title="A lead sent changes for approval" role="status">
      Review them from the Overview. Publishing here instead replaces their request.
      {#snippet actions()}
        <Button variant="outline" size="sm" href="/portal/">Go to Overview</Button>
      {/snippet}
    </Callout>
  {/if}
{:else if approval.status === "sent_back"}
  <Callout
    tone="warning"
    title={`Sent back by ${approval.decidedBy?.name ?? "a director"}${approval.decidedAt === null ? "" : ` ${formatRelative(approval.decidedAt)}`}`}
    role="status"
  >
    {#if approval.note !== null && approval.note.trim() !== ""}
      <q class="note">{approval.note}</q>
    {/if}
    {isLead
      ? " Make the changes, then send it for approval again."
      : " The lead can make the changes and send it again."}
  </Callout>
{/if}

<style>
  .note {
    font-style: italic;
  }
</style>
