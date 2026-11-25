<script lang="ts">
  // Projects in the portal (Figma 107:5109; spec PR-1): search, committee chips with counts, the
  // Drafts / Published / Archived filter and a row per project with Edit. "Add project" opens a
  // dialog that creates a draft and goes straight to its editor; `?add=1` (the overview's Add
  // project) opens it on arrival. Filters live in the address.
  import { onDestroy, onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import Lock from "@lucide/svelte/icons/lock";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";
  import SquareKanban from "@lucide/svelte/icons/square-kanban";
  import { get, isApiError, post } from "#lib/api/client.ts";
  import type {
    CommitteeSlug,
    ProjectAdminSummary,
    ProjectEditorResponse,
    ProjectListResponse,
  } from "#lib/api/types.ts";
  import { COMMITTEES, COMMITTEE_LIST, isCommitteeSlug } from "#lib/portal/committees.ts";
  import { formatAbsolute, formatUpdated } from "#lib/portal/dates.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import {
    Button,
    Callout,
    CommitteeChip,
    Dialog,
    EmptyState,
    Field,
    PageHeader,
    SegmentedControl,
    Select,
    Skeleton,
    StatusDot,
    TextInput,
  } from "#lib/portal/ui/index.ts";
  import Img from "#lib/media/Img.svelte";
  import {
    MAX_NAME_CHARS,
    type StateFilter,
    filterAnnouncement,
    isStateFilter,
    matchesRow,
    projectCount,
    stateLabel,
  } from "#lib/projects/drafts.ts";

  const session = getPortalSession();
  const permissions = $derived(session.permissions);
  const addableCommittees = $derived(permissions?.projectCommittees ?? []);

  let rows = $state.raw<ProjectAdminSummary[] | null>(null);
  let loadError = $state<string | null>(null);

  const committee = $derived.by((): CommitteeSlug | null => {
    const value = page.url.searchParams.get("committee");
    return isCommitteeSlug(value) ? value : null;
  });
  const stateFilter = $derived.by((): StateFilter => {
    const value = page.url.searchParams.get("state");
    return isStateFilter(value) ? value : "all";
  });
  let search = $state(page.url.searchParams.get("q") ?? "");
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  const STATE_OPTIONS = [
    { value: "all", label: "All" },
    { value: "published", label: "Published" },
    { value: "draft", label: "Drafts" },
    { value: "archived", label: "Archived" },
  ] as const satisfies readonly { value: StateFilter; label: string }[];

  // Rows matching the search and state; the committee chips count these.
  const inScope = $derived(
    (rows ?? []).filter(
      (row) => (stateFilter === "all" || row.state === stateFilter) && matchesRow(row, search),
    ),
  );
  const shown = $derived(committee === null ? inScope : inScope.filter((row) => row.committee === committee));
  const chips = $derived(
    COMMITTEE_LIST.map((style) => ({
      slug: style.slug as CommitteeSlug,
      count: inScope.filter((row) => row.committee === style.slug).length,
    }))
      .filter((chip) => chip.count > 0 || chip.slug === committee)
      .toSorted((a, b) => b.count - a.count),
  );
  const filtered = $derived(committee !== null || stateFilter !== "all" || search.trim() !== "");
  // Directors see every project but edit only their committee's.
  const anyLocked = $derived(shown.some((row) => !row.canEdit));

  // Screen readers hear how many projects are left once the filters or search settle, the way
  // the table and the empty state show it.
  const SETTLE_MS = 400;
  let announced = $state("");
  let announceTimer: ReturnType<typeof setTimeout> | undefined;
  let announcedFilters: string | null = null;

  $effect(() => {
    const key = `${committee ?? ""}|${stateFilter}|${search.trim().toLowerCase()}`;
    const total = rows?.length ?? null;
    const count = shown.length;
    if (total === null) return;
    // The first list after loading is the page itself, which the shell already announced.
    if (announcedFilters === null || announcedFilters === key) {
      announcedFilters = key;
      return;
    }
    announcedFilters = key;
    clearTimeout(announceTimer);
    announceTimer = setTimeout(() => (announced = filterAnnouncement(count, total)), SETTLE_MS);
  });

  onDestroy(() => {
    clearTimeout(announceTimer);
    clearTimeout(searchTimer);
  });

  function lockReason(row: ProjectAdminSummary): string {
    return `Only the ${COMMITTEES[row.committee].name} director or an admin can edit ${row.name}.`;
  }

  onMount(() => {
    void load();
  });

  async function load(): Promise<void> {
    loadError = null;
    try {
      rows = (await get<ProjectListResponse>("/projects")).projects;
    } catch (error) {
      loadError = isApiError(error) ? error.message : "We couldn\u2019t load the projects. Try again.";
    }
  }

  function setParams(changes: Record<string, string | null>): void {
    const params = new URLSearchParams(page.url.searchParams.toString());
    for (const [key, value] of Object.entries(changes)) {
      if (value === null || value === "" || value === "all") params.delete(key);
      else params.set(key, value);
    }
    const query = params.toString();
    // Same page, new filters: keep focus and scroll where they are.
    void goto(query === "" ? page.url.pathname : `?${query}`, { replace: true, reset: false });
  }

  function onsearch(): void {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => setParams({ q: search.trim() }), 300);
  }

  function clearFilters(): void {
    search = "";
    setParams({ committee: null, state: null, q: null });
  }

  function linksText(row: ProjectAdminSummary): string {
    if (row.hasRepo && row.hasSite) return "Repository and live site";
    if (row.hasRepo) return "Repository, no live site";
    if (row.hasSite) return "Live site, no repository";
    return "No links yet";
  }

  // Adding a project

  let adding = $state(false);
  let newName = $state("");
  let newCommittee = $state<CommitteeSlug | "">("");
  let addErrors = $state<Record<string, string>>({});
  let addError = $state<string | null>(null);
  let creating = $state(false);

  function openAdd(): void {
    newName = "";
    newCommittee =
      committee !== null && addableCommittees.includes(committee) ? committee : (addableCommittees[0] ?? "");
    addErrors = {};
    addError = null;
    adding = true;
  }

  // `?add=1` opens the dialog for people who may add projects, then leaves the address, so going
  // back or reloading does not open it again.
  $effect(() => {
    if (!page.url.searchParams.has("add") || permissions === null) return;
    untrack(() => {
      if (addableCommittees.length > 0) openAdd();
      setParams({ add: null });
    });
  });

  async function create(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    addErrors = {};
    addError = null;
    if (newName.trim() === "") {
      addErrors = { name: "Add the project name." };
      return;
    }
    if (newCommittee === "") {
      addErrors = { committee: "Choose the committee that owns the project." };
      return;
    }
    creating = true;
    try {
      const created = await post<ProjectEditorResponse>("/projects", {
        name: newName.trim(),
        committee: newCommittee,
      });
      adding = false;
      await goto(`/portal/projects/${created.project.id}/`);
    } catch (error) {
      if (isApiError(error) && Object.keys(error.fields).length > 0) addErrors = { ...error.fields };
      else addError = isApiError(error) ? error.message : "We couldn\u2019t add the project. Try again.";
    } finally {
      creating = false;
    }
  }
</script>

<svelte:head>
  <title>Projects - ScottyLabs Portal</title>
</svelte:head>

<PageHeader title="Projects" description="Everything on the Projects page. Each project has its own page.">
  {#snippet actions()}
    {#if addableCommittees.length > 0}
      <Button icon={Plus} size="lg" onclick={openAdd}>Add project</Button>
    {/if}
  {/snippet}
</PageHeader>

<div class="toolbar">
  <div class="toolbar__search p-fields-compact">
    <TextInput
      bind:value={search}
      icon={Search}
      type="search"
      placeholder="Search projects"
      aria-label="Search projects"
      oninput={onsearch}
    />
  </div>
  <div class="toolbar__chips" role="group" aria-label="Filter by committee">
    <button
      type="button"
      class={["all-chip", committee === null && "all-chip--selected"]}
      aria-pressed={committee === null ? "true" : "false"}
      onclick={() => setParams({ committee: null })}
    >
      All <span class="all-chip__count" aria-hidden="true">{inScope.length}</span>
      <span class="p-sr-only">, {projectCount(inScope.length)}</span>
    </button>
    {#each chips as chip (chip.slug)}
      <CommitteeChip
        committee={chip.slug}
        selectable
        selected={committee === chip.slug}
        count={chip.count}
        countLabel={projectCount(chip.count)}
        onclick={() => setParams({ committee: committee === chip.slug ? null : chip.slug })}
      />
    {/each}
  </div>
  <SegmentedControl
    label="Show projects"
    options={STATE_OPTIONS}
    value={stateFilter}
    size="sm"
    onchange={(value) => setParams({ state: value })}
  />
</div>

<p class="p-sr-only" aria-live="polite">{announced}</p>

{#if loadError !== null}
  <Callout tone="error" title="We couldn&rsquo;t load the projects" role="alert">
    {loadError}
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={() => void load()}>Try again</Button>
    {/snippet}
  </Callout>
{:else if rows === null}
  <div class="p-table-card" aria-busy="true">
    <p class="p-sr-only" role="status">Loading projects</p>
    <table class="p-table" aria-hidden="true">
      <tbody>
        {#each [0, 1, 2, 3, 4] as item (item)}
          <tr>
            <td><div class="p-table__primary"><Skeleton width="64px" height="48px" radius="8px" /><Skeleton width="160px" height="14px" /></div></td>
            <td><Skeleton width="64px" height="20px" radius="999px" /></td>
            <td><Skeleton width="90px" height="14px" /></td>
            <td><Skeleton width="60px" height="14px" /></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else if rows.length === 0}
  <div class="p-table-card">
    {#if addableCommittees.length > 0}
      <EmptyState
        icon={SquareKanban}
        title="No projects yet"
        description="Add the first one. It stays a draft, seen only in the portal, until you publish it."
      >
        {#snippet actions()}
          <Button icon={Plus} onclick={openAdd}>Add project</Button>
        {/snippet}
      </EmptyState>
    {:else}
      <EmptyState
        icon={SquareKanban}
        title="No projects to edit yet"
        description="When the director of your committee adds you to a project, it shows up here."
      />
    {/if}
  </div>
{:else if shown.length === 0}
  <div class="p-table-card">
    <EmptyState
      icon={Search}
      title="No projects match these filters"
      description="Try another committee or search, or clear the filters to see every project."
    >
      {#snippet actions()}
        <Button variant="outline" onclick={clearFilters}>Clear filters</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <div class="p-table-card">
    <table class="p-table p-table--stack">
      <thead>
        <tr>
          <th scope="col">Project</th>
          <th scope="col">Committee</th>
          <th scope="col">Status</th>
          <th scope="col">Links</th>
          <th scope="col">Updated</th>
          <th scope="col"><span class="p-sr-only">Actions</span></th>
        </tr>
      </thead>
      <tbody>
        {#each shown as row (row.id)}
          {@const status = stateLabel(row)}
          <tr>
            <td data-label="Project" class="p-table__cell--full">
              <div class="p-table__primary">
                <span class="thumb" style:--tint={COMMITTEES[row.committee].tint}>
                  {#if row.cover !== null}
                    <Img image={row.cover} alt="" sizes="64px" fallbackWidth={480} fit="cover" />
                  {/if}
                </span>
                <div class="p-table__title">
                  <span class="name">{row.name}</span>
                  <span class="p-table__subtitle">/projects/{row.slug}</span>
                </div>
              </div>
            </td>
            <td data-label="Committee"><CommitteeChip committee={row.committee} /></td>
            <td data-label="Status">
              <div class="status">
                <StatusDot tone={status.tone} label={status.label} />
                {#if row.pendingApproval}
                  <span class="status__note">Waiting for approval</span>
                {/if}
                {#if row.featured}
                  <span class="status__note">Featured</span>
                {/if}
              </div>
            </td>
            <td data-label="Links">
              <span class="links" title={linksText(row)}>
                <GitBranch class={row.hasRepo ? "links__icon" : "links__icon links__icon--off"} size={16} strokeWidth={1.75} aria-hidden="true" />
                <ExternalLink class={row.hasSite ? "links__icon" : "links__icon links__icon--off"} size={16} strokeWidth={1.75} aria-hidden="true" />
                <span class="p-sr-only">{linksText(row)}</span>
              </span>
            </td>
            <td data-label="Updated" class="p-table__muted">
              <time datetime={row.updatedAt} title={formatAbsolute(row.updatedAt)}>{formatUpdated(row.updatedAt)}</time>
              {#if row.updatedBy !== null}<span class="p-sr-only"> by {row.updatedBy.name}</span>{/if}
            </td>
            <td class="p-table__actions">
              {#if row.canEdit}
                <Button variant="outline" size="sm" href={`/portal/projects/${row.id}/`} aria-label={`Edit ${row.name}`}>
                  Edit
                </Button>
              {:else}
                <span class="locked" title={lockReason(row)}>
                  <Lock size={16} strokeWidth={2} aria-hidden="true" />
                  <span class="p-sr-only">{lockReason(row)}</span>
                  <Button variant="subtle" size="sm" href={`/portal/projects/${row.id}/`} aria-label={`View ${row.name}`}>
                    View
                  </Button>
                </span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <p class="p-table-footnote">
    Showing {shown.length} of {rows.length}.
    {#if anyLocked}Only the director of a project&rsquo;s committee, or an admin, can edit it.{/if}
    {#if filtered}<button type="button" class="reset" onclick={clearFilters}>Clear filters</button>{/if}
  </p>
{/if}

<Dialog
  bind:open={adding}
  title="Add a project"
  description="It starts as a draft that only people in the portal can see. Publish it when it is ready."
  dismissible={!creating}
>
  <form id="add-project" class="add-form" onsubmit={create} novalidate>
    {#if addError !== null}
      <Callout tone="error" role="alert">{addError}</Callout>
    {/if}
    <Field
      label="Project name"
      error={addErrors.name}
      counter={{ length: newName.trim().length, max: MAX_NAME_CHARS }}
      required
    >
      <TextInput bind:value={newName} maxlength={MAX_NAME_CHARS + 20} autocomplete="off" />
    </Field>
    <Field
      label="Committee"
      help="The committee that owns the project. Its director publishes it."
      error={addErrors.committee}
      required
    >
      <Select
        bind:value={newCommittee}
        options={addableCommittees.map((slug) => ({ value: slug, label: COMMITTEES[slug].name }))}
        disabled={addableCommittees.length < 2}
      />
    </Field>
  </form>
  {#snippet footer()}
    <Button variant="outline" disabled={creating} onclick={() => (adding = false)}>Cancel</Button>
    <Button type="submit" form="add-project" loading={creating}>Add project</Button>
  {/snippet}
</Dialog>

<style>
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    margin: 28px 0 24px;
  }
  .toolbar__search {
    flex: 0 1 280px;
    min-width: 200px;
  }
  .toolbar__chips {
    display: flex;
    flex: 1 1 auto;
    flex-wrap: wrap;
    gap: 6px;
  }
  .all-chip {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 11px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-pill);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .all-chip--selected {
    border-color: var(--p-inverse);
    background: var(--p-inverse);
    color: var(--p-on-inverse);

    &:hover {
      background: var(--p-inverse-hover);
    }
    & .all-chip__count {
      color: #d6d6d6;
    }
  }
  .all-chip__count {
    color: #555555;
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }

  .thumb {
    position: relative;
    display: block;
    flex: none;
    overflow: hidden;
    width: 64px;
    height: 48px;
    border-radius: 8px;
    background: var(--tint);

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
    }
  }
  .name {
    font-weight: 500;
  }
  .status {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .status__note {
    padding-left: 16px;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .links {
    display: inline-flex;
    gap: 10px;
    color: var(--p-text);

    & :global(.links__icon--off) {
      color: #c2c2c2;
    }
  }
  .locked {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    color: var(--p-text-secondary);
  }
  .reset {
    margin-left: 6px;
    color: var(--p-text);
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
      border-radius: 2px;
    }
  }
  .add-form {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  @media (max-width: 600px) {
    .toolbar__search {
      flex: 1 1 100%;
    }
  }
</style>
