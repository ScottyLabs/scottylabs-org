<script lang="ts" module>
  import type { CommitteeSlug, Role } from "#lib/api/types.ts";
  import type { AssignmentRow } from "./people.ts";

  /**
   * What the dialog does: add someone (admins choose any role; directors add leads to the
   * committees they direct), move a role to another role, or change a lead's projects and ticker
   * permission.
   */
  export type RoleDialogMode =
    | { kind: "add"; committees: readonly CommitteeSlug[] | "any" }
    | { kind: "change"; row: AssignmentRow; role: Role }
    | { kind: "edit"; row: AssignmentRow };
</script>

<script lang="ts">
  // Adding a person or changing a role (Figma 111:9332 for admins, 111:9029 for directors).
  // The service has the last word on every rule; its messages appear next to the field they are
  // about, or above the buttons.
  import Lock from "@lucide/svelte/icons/lock";
  import { isApiError, patch, post } from "#lib/api/client.ts";
  import type { AddPersonInput, PersonRow, ProjectAdminSummary, UpdateAssignmentInput } from "#lib/api/types.ts";
  import { COMMITTEE_SLUGS } from "#lib/api/types.ts";
  import { committeeName } from "#lib/portal/committees.ts";
  import {
    Button,
    Callout,
    Checkbox,
    Dialog,
    Field,
    PersonPicker,
    SegmentedControl,
    Select,
    TextInput,
    type PickedPerson,
    type SelectOption,
  } from "#lib/portal/ui/index.ts";
  import ProjectChecklist from "./ProjectChecklist.svelte";
  import { clearedProjectsNote, matchingAssignment, nowHasRole, projectsFor, rolePhrase } from "./people.ts";

  let {
    open = $bindable(false),
    mode,
    projects,
    projectsError = null,
    onretryProjects,
    excludeFor,
    onsaved,
  }: {
    open?: boolean;
    mode: RoleDialogMode | null;
    /** Every project, for the lead checkboxes; null while loading or when it failed. */
    projects: readonly ProjectAdminSummary[] | null;
    projectsError?: string | null;
    onretryProjects?: () => void;
    /** Andrew IDs the person picker leaves out for a committee. */
    excludeFor?: (committee: CommitteeSlug | null) => string[];
    /** The person's updated row, the confirmation to show, and the role to highlight. */
    onsaved: (row: PersonRow, message: string, highlight: string | undefined) => void;
  } = $props();

  const ROLE_OPTIONS: readonly SelectOption<Role>[] = [
    { value: "lead", label: "Lead" },
    { value: "director", label: "Director" },
    { value: "admin", label: "Admin" },
  ];

  let person = $state<PickedPerson | null>(null);
  let chosenRole = $state<Role>("lead");
  let chosenCommittee = $state<CommitteeSlug | "">("");
  let selectedProjects = $state<string[]>([]);
  let canPostTicker = $state(true);
  let errors = $state<Record<string, string>>({});
  let failure = $state<string | null>(null);
  let saving = $state(false);

  // Each time it opens, start from the person and role it is about.
  let shownMode: RoleDialogMode | null = null;
  $effect(() => {
    if (!open || mode === shownMode) return;
    shownMode = mode;
    reset(mode);
  });

  function reset(next: RoleDialogMode | null) {
    errors = {};
    failure = null;
    saving = false;
    person = null;
    chosenRole = "lead";
    chosenCommittee = "";
    selectedProjects = [];
    canPostTicker = true;
    if (next === null) return;
    if (next.kind === "add") {
      if (next.committees !== "any" && next.committees.length === 1) chosenCommittee = next.committees[0] ?? "";
      return;
    }
    const { assignment } = next.row;
    chosenCommittee = assignment.committee ?? "";
    canPostTicker = assignment.canPostTicker;
    selectedProjects = [...assignment.projectIds];
  }

  const adminAdds = $derived(mode?.kind === "add" && mode.committees === "any");
  const role = $derived.by<Role>(() => {
    if (mode === null) return "lead";
    if (mode.kind === "add") return adminAdds ? chosenRole : "lead";
    if (mode.kind === "change") return mode.role;
    return mode.row.assignment.role;
  });
  const committeeChoices = $derived.by<readonly CommitteeSlug[]>(() =>
    mode?.kind === "add" && mode.committees !== "any" ? mode.committees : COMMITTEE_SLUGS,
  );
  const committeeOptions = $derived<SelectOption<CommitteeSlug | "">[]>(
    committeeChoices.map((slug) => ({ value: slug, label: committeeName(slug) })),
  );
  const showCommitteeSelect = $derived(
    role !== "admin" &&
      mode !== null &&
      (mode.kind === "change" || (mode.kind === "add" && committeeChoices.length > 1)),
  );
  const committee = $derived<CommitteeSlug | null>(
    role === "admin" || chosenCommittee === "" ? null : chosenCommittee,
  );
  const projectsLoaded = $derived(projects !== null && projectsError === null);
  const projectOptions = $derived(projects === null ? null : projectsFor(projects, committee));
  const personName = $derived(mode !== null && mode.kind !== "add" ? mode.row.person.name : "");

  const title = $derived.by(() => {
    if (mode === null) return "Add a person";
    if (mode.kind === "add") {
      if (adminAdds) return "Add a person";
      return committeeChoices.length === 1 && committee !== null
        ? `Add a lead to ${committeeName(committee)}`
        : "Add a lead";
    }
    if (mode.kind === "change") {
      const article = mode.role === "admin" ? "an admin" : `a ${mode.role}`;
      return `Make ${personName} ${article}`;
    }
    return `What ${personName} can edit`;
  });

  const description = $derived.by(() => {
    if (mode === null || mode.kind === "add") {
      return adminAdds
        ? "Admins can add leads, directors and other admins."
        : "They get access the next time they open the portal.";
    }
    if (mode.kind === "edit") {
      return `As ${rolePhrase(mode.row.assignment.role, mode.row.assignment.committee)}.`;
    }
    switch (mode.role) {
      case "admin":
        return "Admins can edit everything, including roles.";
      case "director":
        return "Directors edit their committee page and projects, post to the ticker and approve what their leads publish.";
      case "lead":
        return "Leads edit the projects you choose and can write ticker announcements that a director approves.";
    }
  });

  const submitLabel = $derived.by(() => {
    if (mode === null || mode.kind === "add") return adminAdds ? "Add person" : "Add lead";
    if (mode.kind === "change") return `Make ${mode.role}`;
    return "Save changes";
  });

  const exclude = $derived(excludeFor?.(committee) ?? []);

  function check(): boolean {
    const found: Record<string, string> = {};
    if (mode?.kind === "add" && person === null) {
      found.andrewId = "Choose a person, or type their Andrew ID.";
    }
    if (role !== "admin" && committee === null) found.committee = "Choose the committee for this role.";
    errors = found;
    return Object.keys(found).length === 0;
  }

  /** The chosen projects that belong to the committee; null leaves a lead's projects as they are. */
  function projectIds(): string[] | null {
    if (role !== "lead") return [];
    if (!projectsLoaded || projectOptions === null) return null;
    const allowed = new Set(projectOptions.map((project) => project.id));
    return selectedProjects.filter((id) => allowed.has(id));
  }

  function request(): Promise<PersonRow> {
    const ids = projectIds();
    if (mode === null || mode.kind === "add") {
      const body: AddPersonInput = {
        andrewId: person?.andrewId ?? "",
        name: null,
        role,
        committee,
        canPostTicker: role === "lead" ? canPostTicker : true,
        projectIds: ids ?? [],
      };
      return post<PersonRow>("/people", body);
    }
    const body: UpdateAssignmentInput =
      mode.kind === "change" ? { role, committee } : {};
    if (role === "lead") {
      body.canPostTicker = canPostTicker;
      if (ids !== null) body.projectIds = ids;
    }
    return patch<PersonRow>(`/people/assignments/${encodeURIComponent(mode.row.assignment.id)}`, body);
  }

  const FIELD_KEYS = ["andrewId", "name", "role", "committee", "projectIds"];

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving || !check()) return;
    failure = null;
    saving = true;
    try {
      const row = await request();
      saving = false;
      open = false;
      const name = row.person.name;
      const message =
        mode?.kind === "edit" ? `Saved what ${name} can edit.` : nowHasRole(name, role, committee);
      onsaved(row, message, matchingAssignment(row, role, committee)?.id);
    } catch (error) {
      saving = false;
      if (!isApiError(error)) {
        failure = "That did not save. Try again in a moment.";
        return;
      }
      const fieldErrors = Object.fromEntries(
        FIELD_KEYS.flatMap((key) => {
          const message = error.field(key);
          return message === undefined ? [] : [[key, message]];
        }),
      );
      errors = fieldErrors;
      if (Object.keys(fieldErrors).length === 0) failure = error.message;
    }
  }

  const formId = $props.id();
</script>

<Dialog bind:open {title} {description} dismissible={!saving}>
  <form id={`role-form-${formId}`} class="role-form" novalidate onsubmit={save}>
    {#if mode?.kind === "add"}
      <Field label="Person" error={errors.andrewId}>
        <PersonPicker bind:value={person} {exclude} disabled={saving} />
      </Field>
    {/if}

    {#if adminAdds}
      <div class="role-form__group">
        <span class="role-form__label" aria-hidden="true">Role</span>
        <SegmentedControl label="Role" options={ROLE_OPTIONS} bind:value={chosenRole} />
        {#if errors.role}<p class="role-form__error" role="alert">{errors.role}</p>{/if}
      </div>
    {:else if mode?.kind === "add"}
      <Field
        label="Role"
        help="Directors appoint leads for their own committee. Admins appoint directors."
      >
        <div class="role-form__locked">
          <TextInput
            value={committeeChoices.length === 1 && committee !== null ? `Lead, in ${committeeName(committee)}` : "Lead"}
            readonly
          />
          <Lock class="role-form__lock" size={16} strokeWidth={1.75} aria-hidden="true" />
        </div>
      </Field>
    {/if}

    {#if showCommitteeSelect || (adminAdds && role !== "admin")}
      <Field label="Committee" error={errors.committee}>
        <Select
          options={committeeOptions}
          bind:value={chosenCommittee}
          placeholder="Choose a committee"
          disabled={saving}
        />
      </Field>
    {/if}

    {#if role === "lead" && committee !== null}
      <ProjectChecklist
        projects={projectOptions}
        {committee}
        bind:selected={selectedProjects}
        loadError={projectsError}
        onretry={onretryProjects}
        error={errors.projectIds}
        disabled={saving}
      />
      <fieldset class="role-form__group role-form__fieldset" disabled={saving}>
        <legend class="role-form__label">Ticker</legend>
        <Checkbox
          bind:checked={canPostTicker}
          label="Can write ticker announcements"
          description="A director approves each one before it goes live."
        />
      </fieldset>
    {:else if role !== "lead" && mode?.kind === "change" && mode.row.assignment.projectIds.length > 0}
      <p class="role-form__note">{clearedProjectsNote(role)}</p>
    {/if}

    {#if failure !== null}
      <Callout tone="error" role="alert" title={failure} />
    {/if}
  </form>

  {#snippet footer()}
    <Button variant="outline" disabled={saving} onclick={() => (open = false)}>Cancel</Button>
    <Button type="submit" form={`role-form-${formId}`} loading={saving}>{submitLabel}</Button>
  {/snippet}
</Dialog>

<style>
  .role-form {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .role-form__group {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    min-width: 0;
  }
  .role-form__fieldset {
    margin: 0;
    padding: 0;
    border: 0;

    & > legend {
      margin-bottom: 12px;
    }
  }
  .role-form__label {
    padding: 0;
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .role-form__error {
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .role-form__locked {
    position: relative;

    & :global(input) {
      padding-right: 40px;
      background: var(--p-surface-muted);
      border-color: var(--p-surface-muted);
    }
    & :global(.role-form__lock) {
      position: absolute;
      top: 50%;
      right: 14px;
      color: var(--p-text);
      transform: translateY(-50%);
      pointer-events: none;
    }
  }
  .role-form__note {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
</style>
