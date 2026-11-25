<script lang="ts">
  // "Projects they can edit": the committee's projects as checkboxes, in the add and edit dialogs
  // (Figma 111:9332, 111:9029). A lead can only be given projects their committee owns.
  import type { ProjectAdminSummary } from "#lib/api/types.ts";
  import { committeeName } from "#lib/portal/committees.ts";
  import { Button, Checkbox, Skeleton } from "#lib/portal/ui/index.ts";
  import type { CommitteeSlug } from "#lib/api/types.ts";

  let {
    projects,
    committee,
    selected = $bindable([]),
    loadError = null,
    onretry,
    error = null,
    disabled = false,
  }: {
    /** The committee's projects, or null while they load. */
    projects: readonly ProjectAdminSummary[] | null;
    committee: CommitteeSlug;
    selected?: string[];
    loadError?: string | null;
    onretry?: () => void;
    error?: string | null;
    disabled?: boolean;
  } = $props();

  const uid = $props.id();
  const errorId = `projects-${uid}-error`;

  function toggle(id: string, checked: boolean) {
    selected = checked ? [...selected.filter((value) => value !== id), id] : selected.filter((value) => value !== id);
  }
</script>

<fieldset class="checklist" aria-describedby={error ? errorId : undefined} {disabled}>
  <legend class="checklist__legend">Projects they can edit</legend>
  {#if loadError !== null}
    <div class="checklist__box checklist__box--message">
      <p class="checklist__message">{loadError}</p>
      {#if onretry}<Button variant="outline" size="sm" onclick={onretry}>Try again</Button>{/if}
    </div>
  {:else if projects === null}
    <div class="checklist__box" aria-hidden="true">
      <Skeleton height="36px" />
      <Skeleton height="36px" />
    </div>
  {:else if projects.length === 0}
    <p class="checklist__box checklist__box--message checklist__message">
      {committeeName(committee)} has no projects yet. You can give them projects later.
    </p>
  {:else}
    <div class="checklist__box">
      {#each projects as project (project.id)}
        <Checkbox
          label={project.name}
          description={`/projects/${project.slug}`}
          checked={selected.includes(project.id)}
          onchange={(event) => toggle(project.id, event.currentTarget.checked)}
        />
      {/each}
    </div>
  {/if}
  {#if error}<p id={errorId} class="checklist__error" role="alert">{error}</p>{/if}
</fieldset>

<style>
  .checklist {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .checklist__legend {
    margin-bottom: 8px;
    padding: 0;
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .checklist__box {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-height: 232px;
    margin: 0;
    padding: 14px 16px;
    overflow-y: auto;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
  }
  .checklist__box--message {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .checklist__message {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .checklist__error {
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.75rem;
    line-height: 16px;
  }
</style>
