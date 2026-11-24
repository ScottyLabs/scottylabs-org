<script lang="ts">
  // Edit an announcement. Admins and directors change live ones at once (spec TK-10); leads edit
  // theirs while they wait, and saving sends the new version for approval.
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { Announcement, AnnouncementInput, AnnouncementListResponse, PersonRef } from "#lib/api/types.ts";
  import { getPortalSession, getToasts } from "#lib/portal/index.ts";
  import { Button, Callout, Skeleton } from "#lib/portal/ui/index.ts";
  import { listNames } from "#lib/ticker/format.ts";
  import AnnouncementForm from "#lib/ticker/portal/AnnouncementForm.svelte";
  import ComposerHeading from "#lib/ticker/portal/ComposerHeading.svelte";
  import { getAnnouncement, listAnnouncements, updateAnnouncement } from "#lib/ticker/portal/api.ts";

  const session = getPortalSession();
  const toasts = getToasts();
  const id = $derived(page.params.id ?? "");

  let announcement = $state.raw<Announcement | null>(null);
  let live = $state.raw<AnnouncementListResponse | null>(null);
  let loadError = $state<string | null>(null);
  let now = $state(new Date());

  $effect(() => {
    const which = id;
    const controller = new AbortController();
    announcement = null;
    loadError = null;
    void load(which, controller.signal);
    return () => controller.abort();
  });

  async function load(which: string, signal: AbortSignal) {
    const liveRequest = listAnnouncements("live", signal).catch(() => null);
    try {
      const found = await getAnnouncement(which, signal);
      live = await liveRequest;
      now = new Date();
      announcement = found;
    } catch (error) {
      if (isAbortError(error)) return;
      loadError = isApiError(error) ? error.message : "We could not load this announcement. Try again.";
    }
  }

  const waiting = $derived(
    announcement !== null && (announcement.state === "pending_approval" || announcement.state === "sent_back"),
  );
  const leftTicker = $derived(announcement !== null && (announcement.state === "expired" || announcement.state === "ended"));
  const canRepost = $derived(
    announcement !== null && session.current.permissions.tickerSources.includes(announcement.committee),
  );

  async function save(input: AnnouncementInput, _sendsForApproval: boolean, approvers: PersonRef[]) {
    if (announcement === null) return;
    const saved = await updateAnnouncement(announcement.id, input);
    if (saved.state === "live") {
      toasts.success("Saved. The ticker shows the change now.", { label: "View", href: "/", external: true });
      await goto(`/portal/ticker/?highlight=${saved.id}`);
      return;
    }
    const names = listNames((saved.approval?.decidedBy ? [saved.approval.decidedBy] : approvers).map((person) => person.name));
    toasts.success(names === "" ? "Sent for approval again." : `Sent to ${names} for approval again.`);
    await goto(`/portal/ticker/?tab=waiting&highlight=${saved.id}`);
  }
</script>

<svelte:head>
  <title>Edit announcement - ScottyLabs Portal</title>
</svelte:head>

<div class="composer-page">
  <ComposerHeading title="Edit announcement" crumb="Edit" />

  {#if loadError !== null}
    <Callout tone="error" role="alert" title="This announcement did not load">
      {loadError}
      {#snippet actions()}<Button variant="outline" size="sm" href="/portal/ticker/">Back to announcements</Button>{/snippet}
    </Callout>
  {:else if announcement === null}
    <div aria-busy="true" aria-label="Loading">
      <Skeleton height="520px" radius="16px" />
    </div>
  {:else if !announcement.canEdit}
    <Callout tone="info" title="You can't edit this announcement">
      {#if leftTicker}
        It has left the ticker. Post it again to put it back.
      {:else}
        Only admins and the director of its committee can change it once it is live.
      {/if}
      {#snippet actions()}
        {#if leftTicker && canRepost}
          <Button size="sm" href={`/portal/ticker/new/?from=${announcement?.id}`}>Post again</Button>
        {/if}
        <Button variant="outline" size="sm" href="/portal/ticker/">Back to announcements</Button>
      {/snippet}
    </Callout>
  {:else}
    {#if announcement.state === "sent_back"}
      <Callout tone="warning" title={announcement.approval?.decidedBy ? `${announcement.approval.decidedBy.name} sent this back` : "This was sent back"}>
        {#if announcement.approval?.note}<p class="composer-page__note">&ldquo;{announcement.approval.note}&rdquo;</p>{/if}
        Change it and send it for approval again.
      </Callout>
    {:else if waiting}
      <Callout tone="info" title="Waiting for approval">
        Saving your changes sends the new version for approval in place of this one.
      </Callout>
    {/if}
    {#key announcement.id}
      <AnnouncementForm editing={announcement} {live} {now} onsave={save} />
    {/key}
  {/if}
</div>

<style>
  .composer-page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .composer-page__note {
    margin: 0 0 4px;
  }
</style>
