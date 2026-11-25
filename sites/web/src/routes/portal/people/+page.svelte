<script lang="ts">
  // People and roles (spec PE-1 to PE-6). Admins see everyone with a role, add people with any
  // role, change and remove roles and sync directors from governance (Figma 107:5980, 111:9332).
  // Directors see their committee's team and add or remove leads (Figma 111:8915, 111:9029,
  // 111:9202). The service decides every rule; its messages are shown as they come.
  import { onMount, tick } from "svelte";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Users from "@lucide/svelte/icons/users";
  import { get, isApiError } from "#lib/api/client.ts";
  import type {
    CommitteeSlug,
    GovernanceSyncResponse,
    PeopleResponse,
    PersonRow,
    ProjectAdminSummary,
    ProjectListResponse,
    Role,
  } from "#lib/api/types.ts";
  import { canSeePeople, committeeNames } from "#lib/portal/index.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import { getToasts } from "#lib/portal/toasts.svelte.ts";
  import { Button, Callout, EmptyState, PageHeader, Skeleton } from "#lib/portal/ui/index.ts";
  import PeopleTable from "./PeopleTable.svelte";
  import RemoveRoleDialog from "./RemoveRoleDialog.svelte";
  import RoleDialog, { type RoleDialogMode } from "./RoleDialog.svelte";
  import RolesPanel from "./RolesPanel.svelte";
  import SyncDialog from "./SyncDialog.svelte";
  import {
    type AssignmentRow,
    assignmentRows,
    peopleIn,
    projectNames,
    withPerson,
    withoutAssignment,
  } from "./people.ts";

  /** How long a row stays marked after it was added or changed. */
  const HIGHLIGHT_MS = 6000;

  const session = getPortalSession();
  const toasts = getToasts();
  const me = $derived(session.me);
  const permissions = $derived(session.permissions);
  const isAdmin = $derived(permissions?.isAdmin ?? false);
  const directs = $derived<CommitteeSlug[]>(permissions?.directorOf ?? []);
  const allowed = $derived(permissions !== null && canSeePeople(permissions));
  const view = $derived<"admin" | "director">(isAdmin ? "admin" : "director");
  const teamName = $derived(committeeNames(directs));

  let people = $state.raw<PersonRow[] | null>(null);
  let loadError = $state<string | null>(null);
  let projects = $state.raw<ProjectAdminSummary[] | null>(null);
  let projectsError = $state<string | null>(null);
  let highlight = $state<string | null>(null);
  let highlightTimer: ReturnType<typeof setTimeout> | undefined;
  let now = $state(new Date());
  let syncMessages = $state<string[]>([]);
  let table = $state<ReturnType<typeof PeopleTable> | null>(null);

  const rows = $derived(
    people === null || me === null
      ? []
      : assignmentRows(people, me.user.id, isAdmin ? "all" : directs),
  );
  const names = $derived(projects === null ? null : projectNames(projects));

  async function loadPeople(): Promise<void> {
    loadError = null;
    try {
      const response = await get<PeopleResponse>("/people");
      people = response.people;
      now = new Date();
    } catch (error) {
      loadError = isApiError(error) ? error.message : "We could not load people. Try again.";
    }
  }

  async function loadProjects(): Promise<void> {
    projectsError = null;
    try {
      const response = await get<ProjectListResponse>("/projects");
      projects = response.projects;
    } catch {
      projects = null;
      projectsError = "We could not load the projects. You can give them projects later.";
    }
  }

  onMount(() => {
    if (!allowed) return;
    void loadPeople();
    void loadProjects();
    return () => clearTimeout(highlightTimer);
  });

  function mark(assignmentId: string | undefined) {
    clearTimeout(highlightTimer);
    highlight = assignmentId ?? null;
    if (assignmentId !== undefined) {
      highlightTimer = setTimeout(() => (highlight = null), HIGHLIGHT_MS);
    }
  }

  /** Roles changed for the signed-in person take effect in this tab at once. */
  function refreshIfMine(personId: string) {
    if (personId === me?.user.id) void session.refresh();
  }

  // ---------------------------------------------------------------------------
  // Dialogs

  let roleOpen = $state(false);
  let roleMode = $state.raw<RoleDialogMode | null>(null);
  let removeOpen = $state(false);
  let removing = $state.raw<AssignmentRow | null>(null);
  let syncOpen = $state(false);

  function openRole(mode: RoleDialogMode) {
    roleMode = mode;
    roleOpen = true;
  }

  function canManage(row: AssignmentRow): boolean {
    if (isAdmin) return true;
    const { role, committee } = row.assignment;
    return role === "lead" && committee !== null && directs.includes(committee) && !row.isMe;
  }

  function excludeFor(committee: CommitteeSlug | null): string[] {
    // Admins may give someone a second committee; a director's picker leaves out their team.
    if (isAdmin || committee === null) return [];
    return peopleIn(rows, committee);
  }

  function saved(row: PersonRow, message: string, assignmentId: string | undefined) {
    people = withPerson(people ?? [], row);
    // So a role added now reads "By you, just now".
    now = new Date();
    toasts.success(message);
    mark(assignmentId);
    refreshIfMine(row.person.id);
  }

  async function removed(row: AssignmentRow, message: string) {
    const index = rows.findIndex((entry) => entry.assignment.id === row.assignment.id);
    people = withoutAssignment(people ?? [], row.assignment.id);
    toasts.success(message);
    refreshIfMine(row.person.id);
    // The role button that opened the dialog went with the row, so the dialog cannot return focus
    // to it. Carry on from the row that took its place, or from the heading.
    await tick();
    if (table?.focusRoleNear(Math.max(index, 0)) !== true) {
      document.querySelector<HTMLElement>("#portal-main h1")?.focus();
    }
  }

  function synced(result: GovernanceSyncResponse) {
    toasts.success(
      `Synced directors: ${result.added} added, ${result.updated} updated, ${result.unchanged} unchanged.`,
    );
    syncMessages = result.messages;
    void loadPeople();
    void session.refresh();
  }

  const title = $derived(isAdmin ? "People and roles" : `${teamName} team`);
  const description = $derived(
    isAdmin
      ? "Who can edit what. A change applies the next time that person opens a page."
      : `Leads you appoint can edit the ${teamName} projects you choose and write ticker announcements. You approve what they publish.`,
  );
</script>

<svelte:head>
  <title>{allowed ? title : "People"} - ScottyLabs Portal</title>
</svelte:head>

{#if !allowed}
  <PageHeader title="People" />
  <EmptyState
    icon={Users}
    title="Only admins and directors manage people."
    description="Ask your director if you need a different role."
  />
{:else}
  <PageHeader {title} {description}>
    {#snippet actions()}
      {#if isAdmin}
        <Button variant="outline" size="lg" icon={RefreshCw} onclick={() => (syncOpen = true)}>
          Sync directors
        </Button>
        <Button size="lg" icon={Plus} onclick={() => openRole({ kind: "add", committees: "any" })}>
          Add person
        </Button>
      {:else}
        <Button size="lg" icon={Plus} onclick={() => openRole({ kind: "add", committees: directs })}>
          Add a lead
        </Button>
      {/if}
    {/snippet}
  </PageHeader>

  <div class="people">
    <div class="people__main">
      {#if syncMessages.length > 0}
        <Callout tone="info" title="What the sync changed">
          <ul class="people__messages">
            {#each syncMessages as message, index (index)}<li>{message}</li>{/each}
          </ul>
          {#snippet actions()}
            <Button variant="outline" size="sm" onclick={() => (syncMessages = [])}>Dismiss</Button>
          {/snippet}
        </Callout>
      {/if}
      {#if loadError !== null}
        <Callout tone="error" role="alert" title={loadError}>
          {#snippet actions()}
            <Button variant="outline" size="sm" onclick={() => void loadPeople()}>Try again</Button>
          {/snippet}
        </Callout>
      {:else if people === null}
        <div class="p-table-card people__loading" aria-busy="true">
          <span class="p-sr-only">Loading people</span>
          {#each [0, 1, 2, 3] as line (line)}<Skeleton height="28px" />{/each}
        </div>
      {:else if rows.length === 0}
        <div class="p-table-card">
          <EmptyState
            icon={Users}
            title={isAdmin ? "Nobody has a role yet." : `Nobody else is on the ${teamName} team yet.`}
            description={isAdmin ? "Add a person to give them a role." : "Add a lead to share the work."}
          />
        </div>
      {:else if me !== null}
        <PeopleTable
          bind:this={table}
          {rows}
          {view}
          projectNames={names}
          viewerId={me.user.id}
          {now}
          {highlight}
          {canManage}
          onchange={(row: AssignmentRow, role: Role) => openRole({ kind: "change", row, role })}
          onedit={(row: AssignmentRow) => openRole({ kind: "edit", row })}
          onremove={(row: AssignmentRow) => {
            removing = row;
            removeOpen = true;
          }}
        />
      {/if}
    </div>
    <RolesPanel {view} committee={teamName} />
  </div>

  <RoleDialog
    bind:open={roleOpen}
    mode={roleMode}
    {projects}
    {projectsError}
    onretryProjects={() => void loadProjects()}
    {excludeFor}
    onsaved={saved}
  />
  <RemoveRoleDialog
    bind:open={removeOpen}
    row={removing}
    actionLabel={isAdmin ? "Remove role" : "Remove lead"}
    onremoved={(row, message) => void removed(row, message)}
  />
  {#if isAdmin}<SyncDialog bind:open={syncOpen} onsynced={synced} />{/if}
{/if}

<style>
  .people {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 1fr);
    gap: 24px;
    align-items: start;
    margin-top: 24px;
  }
  .people__main {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .people__loading {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 20px;
  }
  .people__messages {
    margin: 0;
    padding-left: 18px;
  }
  @media (max-width: 1023px) {
    .people {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
