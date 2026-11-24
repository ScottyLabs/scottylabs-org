<script lang="ts">
  // New announcement, or "Post again" for one that left the ticker (?from=<id>).
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { isApiError } from "#lib/api/client.ts";
  import type { AnnouncementInput, AnnouncementListResponse, PersonRef } from "#lib/api/types.ts";
  import { getPortalSession, getToasts } from "#lib/portal/index.ts";
  import { Button, Callout, Skeleton } from "#lib/portal/ui/index.ts";
  import { listNames } from "#lib/ticker/format.ts";
  import AnnouncementForm from "#lib/ticker/portal/AnnouncementForm.svelte";
  import ComposerHeading from "#lib/ticker/portal/ComposerHeading.svelte";
  import { createAnnouncement, getAnnouncement, listAnnouncements } from "#lib/ticker/portal/api.ts";

  const session = getPortalSession();
  const toasts = getToasts();
  const canPost = $derived(session.current.permissions.tickerSources.length > 0);
  const from = page.url.searchParams.get("from");
  const now = new Date();

  let live = $state.raw<AnnouncementListResponse | null>(null);
  let initial = $state.raw<Partial<AnnouncementInput> | null>(null);
  let ready = $state(false);
  let loadError = $state<string | null>(null);

  onMount(() => {
    const controller = new AbortController();
    void load(controller.signal);
    return () => controller.abort();
  });

  async function load(signal: AbortSignal) {
    // The slots and approvers only add detail; the form works without them.
    const liveRequest = listAnnouncements("live", signal).catch(() => null);
    try {
      if (from !== null) {
        const original = await getAnnouncement(from, signal);
        initial = {
          kind: original.kind,
          committee: original.committee,
          message: original.message,
          linkUrl: original.linkUrl,
        };
      }
    } catch (error) {
      if (signal.aborted) return;
      loadError = isApiError(error) ? error.message : "We could not load the announcement to post again.";
    }
    live = await liveRequest;
    if (!signal.aborted) ready = true;
  }

  async function save(input: AnnouncementInput, sendsForApproval: boolean, approvers: PersonRef[]) {
    const created = await createAnnouncement(input);
    if (sendsForApproval) {
      const names = listNames(approvers.map((person) => person.name));
      toasts.success(names === "" ? "Sent for approval." : `Sent to ${names} for approval.`);
      await goto(`/portal/ticker/?tab=waiting&highlight=${created.id}`);
    } else {
      toasts.success("Published. It is on the home page now.", { label: "View", href: "/", external: true });
      await goto(`/portal/ticker/?highlight=${created.id}`);
    }
  }
</script>

<svelte:head>
  <title>New announcement - ScottyLabs Portal</title>
</svelte:head>

<div class="composer-page">
  <ComposerHeading title={from === null ? "New announcement" : "Post an announcement again"} crumb={from === null ? "New" : "Post again"} />

  {#if !canPost}
    <Callout tone="info" title="You can't post to the ticker yet">
      Ask the director of your committee to let you write announcements.
      {#snippet actions()}<Button variant="outline" size="sm" href="/portal/ticker/">Back to announcements</Button>{/snippet}
    </Callout>
  {:else if !ready}
    <div class="composer-page__loading" aria-busy="true" aria-label="Loading">
      <Skeleton height="520px" radius="16px" />
      <Skeleton height="220px" radius="16px" />
    </div>
  {:else}
    {#if loadError !== null}
      <Callout tone="warning">{loadError} You can still write a new one below.</Callout>
    {/if}
    <AnnouncementForm {initial} {live} {now} onsave={save} />
  {/if}
</div>

<style>
  .composer-page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .composer-page__loading {
    display: grid;
    grid-template-columns: minmax(0, 660px) minmax(0, 1fr);
    gap: 32px;
    align-items: start;
  }
  @media (max-width: 960px) {
    .composer-page__loading {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
