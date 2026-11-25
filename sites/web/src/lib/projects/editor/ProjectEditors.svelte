<script lang="ts">
  // Who may edit the project besides admins and its committee's directors: leads of the owning
  // committee, whose changes wait for approval. Shown to directors and admins.
  import { onMount } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import { get, isApiError, put } from "../../api/client.ts";
  import type {
    CommitteeSlug,
    PeopleResponse,
    PersonRef,
    ProjectEditorResponse,
  } from "../../api/types.ts";
  import { COMMITTEES } from "../../portal/committees.ts";
  import { getToasts } from "../../portal/toasts.svelte.ts";
  import {
    Avatar,
    Field,
    IconButton,
    type PickedPerson,
    PersonPicker,
    isKnownPerson,
    personName,
  } from "../../portal/ui/index.ts";

  let {
    projectId,
    projectName,
    committee,
    editors,
    onchange,
  }: {
    projectId: string;
    projectName: string;
    committee: CommitteeSlug;
    editors: readonly PersonRef[];
    onchange: (response: ProjectEditorResponse) => void;
  } = $props();

  const toasts = getToasts();
  let leads = $state.raw<PersonRef[] | null>(null);
  /** Everyone with a role in the committee who is not a lead there, so the picker leaves them out. */
  let notLeads = $state.raw<string[]>([]);
  let picked = $state<PickedPerson | null>(null);
  let error = $state<string | null>(null);
  let saving = $state(false);

  const committeeName = $derived(COMMITTEES[committee].name);
  const exclude = $derived([...editors.map((person) => person.id), ...notLeads]);

  onMount(() => {
    void loadLeads();
  });

  async function loadLeads(): Promise<void> {
    try {
      const response = await get<PeopleResponse>("/people", { query: { committee } });
      const isLead = (row: PeopleResponse["people"][number]) =>
        row.roles.some((role) => role.role === "lead" && role.committee === committee);
      leads = response.people.filter(isLead).map((row) => row.person);
      notLeads = response.people.filter((row) => !isLead(row)).map((row) => row.person.id);
    } catch {
      // The server still refuses anyone who is not a lead; this only narrows the search.
      leads = [];
    }
  }

  async function save(userIds: string[], done: string): Promise<boolean> {
    saving = true;
    error = null;
    try {
      const response = await put<ProjectEditorResponse>(`/projects/${projectId}/editors`, { userIds });
      onchange(response);
      toasts.success(done);
      return true;
    } catch (failure) {
      error = isApiError(failure)
        ? (failure.field("userIds") ?? failure.message)
        : "We couldn\u2019t change the editors. Try again.";
      return false;
    } finally {
      saving = false;
    }
  }

  async function add(person: PickedPerson | null): Promise<void> {
    if (person === null) return;
    const lead = isKnownPerson(person) ? leads?.find((candidate) => candidate.id === person.id) : undefined;
    if (lead === undefined) {
      error = `${personName(person)} is not a lead in ${committeeName}. Add them as a lead on the People page first.`;
      picked = null;
      return;
    }
    const added = await save(
      [...editors.map((editor) => editor.id), lead.id],
      `${lead.name} can now edit ${projectName}.`,
    );
    if (added) picked = null;
  }

  function remove(person: PersonRef): void {
    void save(
      editors.filter((editor) => editor.id !== person.id).map((editor) => editor.id),
      `${person.name} can no longer edit ${projectName}.`,
    );
  }
</script>

<section class="editors" aria-labelledby="editors-heading">
  <div class="editors__intro">
    <h2 class="editors__heading" id="editors-heading">Editors</h2>
    <p class="editors__text">
      Leads in {committeeName} you add here can edit this project. What they publish waits for your
      approval.
    </p>
  </div>

  {#if editors.length > 0}
    <ul class="editors__list" role="list">
      {#each editors as person (person.id)}
        <li class="editors__person">
          <Avatar initials={person.initials} {committee} />
          <span class="editors__name">
            {person.name}
            <span class="editors__andrew">{person.andrewId}</span>
          </span>
          <IconButton
            label={`Remove ${person.name} as an editor`}
            icon={X}
            size="sm"
            disabled={saving}
            onclick={() => remove(person)}
          />
        </li>
      {/each}
    </ul>
  {:else}
    <p class="editors__text">No leads can edit it yet.</p>
  {/if}

  <Field label="Add an editor" error={error} help={leads !== null && leads.length === 0 ? `${committeeName} has no leads yet. Add them on the People page.` : undefined}>
    <PersonPicker
      bind:value={picked}
      onchange={(person) => void add(person)}
      placeholder={`Search leads in ${committeeName}`}
      allowAndrewId={false}
      {exclude}
      disabled={saving}
    />
  </Field>
</section>

<style>
  .editors {
    display: flex;
    flex-direction: column;
    gap: 16px;
    color: var(--p-text);
  }
  .editors__intro {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .editors__heading {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .editors__text {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .editors__list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
  }
  .editors__person {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;

    & + & {
      border-top: 1px solid var(--p-border-muted);
    }
  }
  .editors__name {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .editors__andrew {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
  }
</style>
