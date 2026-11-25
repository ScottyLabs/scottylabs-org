<script lang="ts">
  // Reviewing what a lead submitted (spec AP-2 to AP-4, Figma 111:9743): exactly what will go
  // live, its details, an optional note, then "Send back" (the note is required) or "Approve
  // and publish". Errors from the service, such as a full ticker, are shown here as they come.
  //   <ApprovalDialog bind:open approval={selected} ondecided={(approval, decision) => ...} />
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import { isApiError } from "../../api/client.ts";
  import type { Approval } from "../../api/types.ts";
  import { RichText } from "../../editor/index.ts";
  import ProjectCard, { PROJECT_KIND_LABELS, PROJECT_STATUS_LABELS } from "../../projects/ProjectCard.svelte";
  import { committeeName, committeeNames } from "../committees.ts";
  import { formatDateTimeLong } from "../dates.ts";
  import { Button, Callout, Dialog, Field, TextArea } from "../ui/index.ts";
  import AnnouncementPreview from "./AnnouncementPreview.svelte";
  import { approve, sendBack } from "./api.ts";
  import {
    ANNOUNCEMENT_KIND_LABELS,
    MAX_NOTE_LENGTH,
    linkHost,
    projectCardFromDoc,
    reviewDescription,
    reviewTitle,
    submissionStatus,
    submitterFirstName,
  } from "./format.ts";

  type Decision = "approved" | "sent_back";

  let {
    open = $bindable(false),
    approval,
    ondecided,
    onfailed,
    onclose,
  }: {
    open?: boolean;
    approval: Approval | null;
    /** Called with the updated approval once the decision is saved. */
    ondecided?: (approval: Approval, decision: Decision) => void;
    /** Called when a decision fails, since the submission may have changed meanwhile. */
    onfailed?: () => void;
    /** Called whenever the dialog closes, decided or not. */
    onclose?: () => void;
  } = $props();

  let note = $state("");
  let noteError = $state<string | null>(null);
  let failure = $state<{ message: string; offerTicker: boolean } | null>(null);
  let busy = $state<Decision | null>(null);
  let noteInput = $state<HTMLTextAreaElement | null>(null);

  // Each review starts with a clean form.
  let wasOpen = false;
  $effect(() => {
    if (open && !wasOpen) {
      note = "";
      noteError = null;
      failure = null;
    }
    wasOpen = open;
  });

  const firstName = $derived(approval === null ? "the lead" : submitterFirstName(approval));
  const noteCounter = $derived(
    note.length > MAX_NOTE_LENGTH - 100 ? { length: note.length, max: MAX_NOTE_LENGTH } : undefined,
  );
  const decided = $derived(approval !== null && !approval.canDecide);

  async function decide(decision: Decision) {
    if (approval === null || busy !== null) return;
    noteError = null;
    failure = null;
    if (decision === "sent_back" && note.trim() === "") {
      noteError = `Write a note so ${firstName} knows what to change.`;
      noteInput?.focus();
      return;
    }
    if (note.trim().length > MAX_NOTE_LENGTH) {
      noteError = `Keep the note to ${MAX_NOTE_LENGTH} characters or fewer.`;
      noteInput?.focus();
      return;
    }
    busy = decision;
    try {
      const updated =
        decision === "approved" ? await approve(approval.id, note) : await sendBack(approval.id, note);
      busy = null;
      open = false;
      ondecided?.(updated, decision);
    } catch (error) {
      busy = null;
      if (isApiError(error) && error.field("note") !== undefined) {
        noteError = error.field("note") ?? null;
        noteInput?.focus();
        return;
      }
      // A full ticker is a conflict; the message says to end one first, so offer the way there.
      failure = {
        message: isApiError(error) ? error.message : "That did not work. Try again in a moment.",
        offerTicker:
          isApiError(error) && error.code === "conflict" && approval.subjectType === "announcement",
      };
      onfailed?.();
    }
  }
</script>

<Dialog
  bind:open
  class="approval-dialog"
  title={approval === null ? "Review" : reviewTitle(approval)}
  description={approval === null ? undefined : reviewDescription(approval)}
  dismissible={busy === null}
  {onclose}
>
  {#if approval !== null}
    {#if approval.announcement !== null}
      {@const announcement = approval.announcement}
      {@const host = linkHost(announcement.linkUrl)}
      <AnnouncementPreview {announcement} />
      <dl class="approval-facts">
        <div><dt>Type</dt><dd>{ANNOUNCEMENT_KIND_LABELS[announcement.kind]}</dd></div>
        <div><dt>From</dt><dd>{committeeName(announcement.committee)}</dd></div>
        <div><dt>Leaves the ticker</dt><dd>{formatDateTimeLong(announcement.expiresAt)}</dd></div>
        <div>
          <dt>Link</dt>
          <dd>
            {#if host !== null && announcement.linkUrl !== null}
              <a class="approval-facts__link" href={announcement.linkUrl} target="_blank" rel="noopener noreferrer">
                {host}<ArrowUpRight size={14} strokeWidth={2} aria-hidden="true" /><span class="p-sr-only">(opens in a new tab)</span>
              </a>
            {:else}
              None
            {/if}
          </dd>
        </div>
      </dl>
    {:else if approval.project !== null}
      {@const project = approval.project}
      {@const site = linkHost(project.siteUrl)}
      {@const repo = linkHost(project.repoUrl)}
      <div class="approval-card">
        <ProjectCard project={projectCardFromDoc(approval.subjectId, project)} href={null} headingLevel={3} loading="eager" />
      </div>
      <dl class="approval-facts">
        <div><dt>Summary</dt><dd>{project.summary === "" ? "None" : project.summary}</dd></div>
        <div><dt>Committee</dt><dd>{committeeName(project.committee)}</dd></div>
        <div>
          <dt>Built with</dt>
          <dd>{project.builtWith.length === 0 ? "None" : committeeNames(project.builtWith)}</dd>
        </div>
        <div>
          <dt>Status and kind</dt>
          <dd>{PROJECT_STATUS_LABELS[project.status]}, {PROJECT_KIND_LABELS[project.kind].toLowerCase()}</dd>
        </div>
        <div>
          <dt>Links</dt>
          <dd>
            {#if site === null && repo === null}
              None
            {:else}
              {#if site !== null && project.siteUrl !== null}
                <a class="approval-facts__link" href={project.siteUrl} target="_blank" rel="noopener noreferrer">
                  {site}<span class="p-sr-only">, the site (opens in a new tab)</span>
                </a>
              {/if}
              {#if repo !== null && project.repoUrl !== null}
                <a class="approval-facts__link" href={project.repoUrl} target="_blank" rel="noopener noreferrer">
                  {repo}<span class="p-sr-only">, the repository (opens in a new tab)</span>
                </a>
              {/if}
            {/if}
          </dd>
        </div>
        <div><dt>Photos</dt><dd>{project.media.length}</dd></div>
      </dl>
      {#if project.description !== ""}
        <section class="approval-description" aria-label="Description">
          <RichText html={project.description} />
        </section>
      {/if}
    {:else}
      <Callout tone="warning" title="We could not show what was submitted.">
        Open it to see it as {firstName} left it.
        {#snippet actions()}<Button variant="outline" size="sm" href={approval.href}>Open it</Button>{/snippet}
      </Callout>
    {/if}

    {#if decided}
      {@const status = submissionStatus(approval)}
      {#if approval.note}
        <Callout tone="info" title={`${status.label}. ${status.detail}.`}>Note: {approval.note}</Callout>
      {:else}
        <Callout tone="info" title={`${status.label}. ${status.detail}.`} />
      {/if}
    {:else}
      <Field label={`Note to ${firstName}`} optional error={noteError} counter={noteCounter}>
        <TextArea
          class="approval-note"
          bind:value={note}
          bind:ref={noteInput}
          rows={1}
          maxlength={MAX_NOTE_LENGTH + 100}
          placeholder="Say what to change if you send it back"
          disabled={busy !== null}
        />
      </Field>
    {/if}

    {#if failure !== null}
      {#if failure.offerTicker}
        <Callout tone="error" role="alert" title={failure.message}>
          {#snippet actions()}
            <Button variant="outline" size="sm" href="/portal/ticker/">Open the ticker</Button>
          {/snippet}
        </Callout>
      {:else}
        <Callout tone="error" role="alert" title={failure.message} />
      {/if}
    {/if}
  {/if}

  {#snippet footer()}
    {#if decided}
      <Button variant="filled" onclick={() => (open = false)}>Close</Button>
    {:else}
      <Button
        variant="outline"
        loading={busy === "sent_back"}
        disabled={busy !== null}
        onclick={() => void decide("sent_back")}
      >
        Send back
      </Button>
      <Button
        variant="filled"
        loading={busy === "approved"}
        disabled={busy !== null}
        onclick={() => void decide("approved")}
      >
        Approve and publish
      </Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  :global(.approval-dialog.p-modal--dialog) {
    --modal-width: 580px;
  }
  .approval-facts {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    font-size: 0.875rem;
    line-height: 20px;

    & > div {
      display: grid;
      grid-template-columns: 150px minmax(0, 1fr);
      gap: 12px;
    }
    & dt {
      color: var(--p-text-tertiary);
    }
    & dd {
      display: flex;
      flex-wrap: wrap;
      gap: 4px 14px;
      min-width: 0;
      margin: 0;
      color: var(--p-text);
      overflow-wrap: anywhere;
    }
    @media (max-width: 480px) {
      & > div {
        grid-template-columns: 1fr;
        gap: 0;
      }
    }
  }
  .approval-facts__link {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    color: var(--p-text);
    text-decoration: underline;
    text-underline-offset: 2px;
    border-radius: 4px;

    &:hover {
      text-decoration-thickness: 2px;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  /* One line, as in the design, growing with what is written. */
  :global(.approval-note.p-textarea) {
    min-height: 42px;
    max-height: 160px;
    field-sizing: content;
    resize: none;
  }
  .approval-description {
    max-width: 62ch;
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 22px;
  }
</style>
