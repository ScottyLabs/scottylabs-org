<script lang="ts">
  // Who may edit the project besides admins and its committee's directors: leads of the owning
  // committee, whose changes wait for approval. Shown to directors and admins. Only those leads
  // are offered, so there is nothing to choose that the server would refuse.
  import { tick } from "svelte";
  import UserPlus from "@lucide/svelte/icons/user-plus";
  import X from "@lucide/svelte/icons/x";
  import { get, isAbortError, isApiError, put } from "../../api/client.ts";
  import type {
    CommitteeSlug,
    PeopleResponse,
    PersonRef,
    ProjectEditorResponse,
  } from "../../api/types.ts";
  import { COMMITTEES } from "../../portal/committees.ts";
  import { getToasts } from "../../portal/toasts.svelte.ts";
  import { Avatar, Button, Field, IconButton, Select } from "../../portal/ui/index.ts";
  import { leadChoices, leadLabel, leadsIn } from "./editors.ts";

  let {
    projectId,
    projectName,
    committee,
    editors,
    onchange,
  }: {
    projectId: string;
    projectName: string;
    /** The saved owner of the project, whose leads can be editors. */
    committee: CommitteeSlug;
    editors: readonly PersonRef[];
    onchange: (response: ProjectEditorResponse) => void;
  } = $props();

  const toasts = getToasts();
  /** The committee's leads; null while they load. */
  let leads = $state.raw<PersonRef[] | null>(null);
  let loadFailed = $state(false);
  let chosen = $state("");
  let error = $state<string | null>(null);
  let saving = $state<"add" | "remove" | null>(null);
  let section: HTMLElement | undefined = $state();

  const committeeName = $derived(COMMITTEES[committee].name);
  const choices = $derived(leads === null ? [] : leadChoices(leads, editors));
  const options = $derived(choices.map((lead) => ({ value: lead.id, label: leadLabel(lead) })));
  const help = $derived(describeChoices());
  const placeholder = $derived(
    leads === null ? "Loading leads" : options.length > 0 ? "Choose a lead" : "No leads to add",
  );

  // The owner can change while the editor is open (an admin moving the project); offer its leads.
  $effect(() => {
    const current = committee;
    const controller = new AbortController();
    void loadLeads(current, controller.signal);
    return () => controller.abort();
  });

  async function loadLeads(current: CommitteeSlug, signal: AbortSignal): Promise<void> {
    leads = null;
    loadFailed = false;
    chosen = "";
    error = null;
    try {
      const response = await get<PeopleResponse>("/people", { query: { committee: current }, signal });
      leads = leadsIn(response.people, current);
    } catch (failure) {
      if (isAbortError(failure)) return;
      leads = [];
      loadFailed = true;
    }
  }

  function describeChoices(): string | undefined {
    if (leads === null) return `Loading the leads in ${committeeName}.`;
    if (loadFailed) return `We couldn\u2019t load the leads in ${committeeName}. Reload the page to try again.`;
    if (leads.length === 0) return `${committeeName} has no leads yet. Add them on the People page first.`;
    if (choices.length === 0) return `Every lead in ${committeeName} can already edit it.`;
    return undefined;
  }

  async function save(userIds: string[], done: string): Promise<boolean> {
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
    }
  }

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const lead = choices.find((candidate) => candidate.id === chosen);
    if (lead === undefined || saving !== null) return;
    saving = "add";
    const added = await save(
      [...editors.map((editor) => editor.id), lead.id],
      `${lead.name || lead.andrewId} can now edit ${projectName}.`,
    );
    saving = null;
    if (added) chosen = "";
    void keepFocus();
  }

  async function remove(person: PersonRef): Promise<void> {
    if (saving !== null) return;
    saving = "remove";
    await save(
      editors.filter((editor) => editor.id !== person.id).map((editor) => editor.id),
      `${person.name || person.andrewId} can no longer edit ${projectName}.`,
    );
    saving = null;
    void keepFocus();
  }

  /** The button pressed can go away or turn off. Keyboard users then continue from the choice of lead, or the heading. */
  async function keepFocus(): Promise<void> {
    await tick();
    const active = document.activeElement;
    const lost =
      active === null ||
      active === document.body ||
      (section?.contains(active) === true && active.matches(":disabled"));
    if (!lost) return;
    const target =
      section?.querySelector<HTMLElement>("select:not(:disabled)") ??
      section?.querySelector<HTMLElement>(".editors__heading");
    target?.focus();
  }
</script>

<section class="editors" aria-labelledby="editors-heading" bind:this={section}>
  <div class="editors__intro">
    <h2 class="editors__heading" id="editors-heading" tabindex="-1">Editors</h2>
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
            label={`Remove ${person.name || person.andrewId} as an editor`}
            icon={X}
            size="sm"
            disabled={saving !== null}
            onclick={() => void remove(person)}
          />
        </li>
      {/each}
    </ul>
  {:else}
    <p class="editors__text">No leads can edit it yet.</p>
  {/if}

  <form class="editors__add" onsubmit={(event) => void add(event)}>
    <Field label="Add an editor" {error} {help}>
      <div class="editors__row">
        <Select
          class="editors__select"
          bind:value={chosen}
          {options}
          {placeholder}
          disabled={options.length === 0 || saving !== null}
        />
        <Button
          type="submit"
          variant="outline"
          icon={UserPlus}
          loading={saving === "add"}
          disabled={chosen === "" || saving !== null}
        >
          Add editor
        </Button>
      </div>
    </Field>
  </form>
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

    &:focus {
      outline: none;
    }
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
  .editors__row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;

    & :global(.editors__select) {
      flex: 1 1 220px;
    }
  }
</style>
