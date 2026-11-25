<script lang="ts">
  // Overview (spec O-3 to O-5, AP-2 to AP-5; Figma 106:4378 admin, 106:4591 director, 111:9582
  // with an approval waiting, 112:9558 phone). What is live in each section, what needs the
  // person, and recent activity, all scoped to their roles. /portal/?review=<approvalId> opens
  // the review dialog for that submission.
  import { onMount, tick, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { get, isApiError } from "#lib/api/client.ts";
  import type { Approval, OverviewResponse } from "#lib/api/types.ts";
  import {
    ApprovalDialog,
    approvedMessage,
    listApprovals,
    sentBackMessage,
  } from "#lib/portal/approvals/index.ts";
  import { formatDay } from "#lib/portal/dates.ts";
  import {
    ActivityList,
    AttentionList,
    CommitteesCard,
    OverviewSkeleton,
    ProjectsCard,
    SubmissionsList,
    TickerCard,
    greeting,
    overviewDescription,
  } from "#lib/portal/overview/index.ts";
  import { PORTAL_HOME } from "#lib/portal/paths.ts";
  import { firstName, primaryRoleLabel } from "#lib/portal/roles.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import { getToasts } from "#lib/portal/toasts.svelte.ts";
  import { Badge, Button, Callout, PageHeader } from "#lib/portal/ui/index.ts";

  /** How many of a lead's own submissions the overview lists. */
  const SUBMISSIONS_SHOWN = 6;

  const session = getPortalSession();
  const toasts = getToasts();
  const me = $derived(session.me);
  const permissions = $derived(session.permissions);
  const role = $derived(me === null ? null : primaryRoleLabel(me.roles));

  let now = $state(new Date());
  let overview = $state.raw<OverviewResponse | null>(null);
  let loadError = $state<string | null>(null);
  let submissions = $state.raw<Approval[]>([]);
  let attentionHeading = $state<HTMLHeadingElement | null>(null);

  async function load(): Promise<void> {
    loadError = null;
    try {
      const [next, mine] = await Promise.all([
        get<OverviewResponse>("/overview"),
        loadSubmissions(),
      ]);
      now = new Date();
      overview = next;
      submissions = mine;
    } catch (error) {
      loadError = isApiError(error)
        ? error.message
        : "We could not load the overview. Check your connection and try again.";
    }
  }

  /** A lead's own submissions. The overview still shows when this list cannot load. */
  async function loadSubmissions(): Promise<Approval[]> {
    if ((session.permissions?.leadOf.length ?? 0) === 0) return [];
    try {
      const [waiting, decided] = await Promise.all([
        listApprovals("waiting", { mine: true }),
        listApprovals("decided", { mine: true }),
      ]);
      return [...waiting.approvals, ...decided.approvals].slice(0, SUBMISSIONS_SHOWN);
    } catch {
      return [];
    }
  }

  onMount(() => {
    void load();
  });

  // ---------------------------------------------------------------------------
  // Reviewing a submission

  const reviewId = $derived(page.url.searchParams.get("review"));
  let reviewing = $state.raw<Approval | null>(null);
  let reviewOpen = $state(false);
  let reviewRequest = 0;

  $effect(() => {
    const id = reviewId;
    if (id === null) {
      reviewOpen = false;
      return;
    }
    untrack(() => void openReview(id));
  });

  async function openReview(id: string): Promise<void> {
    const request = ++reviewRequest;
    if ((session.permissions?.approvesFor.length ?? 0) === 0) {
      toasts.info("Only directors and admins review submissions.");
      clearReview();
      return;
    }
    try {
      const found = await findApproval(id);
      if (request !== reviewRequest) return;
      if (found === null) {
        toasts.info("That submission is not waiting for you. It may have been decided or withdrawn.");
        clearReview();
        return;
      }
      reviewing = found;
      reviewOpen = true;
    } catch (error) {
      if (request !== reviewRequest) return;
      toasts.error(isApiError(error) ? error.message : "We could not open that submission. Try again.");
      clearReview();
    }
  }

  /** Waiting submissions first; a decided one still opens, read only, from an old link. */
  async function findApproval(id: string): Promise<Approval | null> {
    const waiting = await listApprovals("waiting");
    const match = waiting.approvals.find((approval) => approval.id === id);
    if (match !== undefined) return match;
    const decided = await listApprovals("decided");
    return decided.approvals.find((approval) => approval.id === id) ?? null;
  }

  /** Set when a decision failed: the submission may have changed, so the lists are refreshed on close. */
  let reviewStale = false;

  function closeReview(): void {
    if (reviewStale) {
      reviewStale = false;
      void load();
    }
    clearReview();
  }

  function clearReview(): void {
    if (!page.url.searchParams.has("review")) return;
    void goto(PORTAL_HOME, { replace: true, reset: false });
  }

  async function onDecided(approval: Approval, decision: "approved" | "sent_back"): Promise<void> {
    toasts.success(decision === "approved" ? approvedMessage(approval) : sentBackMessage(approval));
    await load();
    await tick();
    // The Review button that opened the dialog is gone now; carry on from the list.
    const active = document.activeElement;
    if (active === null || active === document.body) attentionHeading?.focus();
  }
</script>

<svelte:head>
  <title>Overview - ScottyLabs Portal</title>
</svelte:head>

{#if me !== null && permissions !== null}
  <PageHeader title={greeting(firstName(me.user), now)} description={overviewDescription(permissions)}>
    {#snippet eyebrow()}
      <span>{formatDay(now)}</span>
      {#if role}<Badge>{role}</Badge>{/if}
    {/snippet}
  </PageHeader>

  <div class="overview" aria-busy={overview === null && loadError === null}>
    {#if loadError !== null}
      <Callout tone="error" role="alert" title={loadError}>
        {#snippet actions()}
          <Button variant="outline" size="sm" onclick={() => void load()}>Try again</Button>
        {/snippet}
      </Callout>
    {:else if overview === null}
      <OverviewSkeleton />
    {:else}
      <div class="overview__grid">
        <div class="overview__cards">
          <TickerCard ticker={overview.ticker} {permissions} {now} />
          <CommitteesCard committees={overview.committees} {permissions} />
          <ProjectsCard projects={overview.projects} {permissions} />
        </div>
        <div class="overview__main">
          <AttentionList items={overview.attention} bind:heading={attentionHeading} />
          {#if submissions.length > 0}<SubmissionsList {submissions} {now} />{/if}
        </div>
        <div class="overview__side">
          <ActivityList entries={overview.activity} viewerId={me.user.id} {now} />
        </div>
      </div>
    {/if}
  </div>
{/if}

<ApprovalDialog
  bind:open={reviewOpen}
  approval={reviewing}
  ondecided={(approval, decision) => void onDecided(approval, decision)}
  onfailed={() => (reviewStale = true)}
  onclose={closeReview}
/>

<style>
  .overview {
    margin-top: 24px;
  }
  .overview__grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 24px;
  }
  /* The three cards share the first row and its height. */
  .overview__cards {
    display: contents;
  }
  .overview__main,
  .overview__side {
    display: flex;
    flex-direction: column;
    gap: 24px;
    align-self: start;
    min-width: 0;
  }
  .overview__main {
    grid-column: span 2;
  }
  /* Phones put what needs the person first, then the sections, then activity (Figma 112:9558). */
  @media (max-width: 899px) {
    .overview {
      margin-top: 20px;
    }
    .overview__grid {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }
    .overview__main {
      grid-column: auto;
      order: -1;
      gap: 16px;
    }
    .overview__side {
      order: 1;
    }
  }
</style>
