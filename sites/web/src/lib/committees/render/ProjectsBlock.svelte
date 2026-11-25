<script lang="ts">
  // The committee's published projects (spec C-8), with the same card as the Projects page.
  import type { BlockByType, ProjectCard as ProjectCardData } from "../../api/types.ts";
  import ProjectCard from "../../projects/ProjectCard.svelte";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";
  import { committeeProjectsPath } from "../links.ts";
  import type { CommitteeInfo } from "./types.ts";

  let {
    props,
    committee,
    projects,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["projects"];
    committee: CommitteeInfo;
    projects: readonly ProjectCardData[];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["projects"]) => void;
  } = $props();
</script>

{#if projects.length > 0 || editor}
  <section class="projects">
    <div class="projects__heading">
      <EditableText
        as="h2"
        class="cb-heading"
        value={props.heading}
        {editor}
        {editable}
        label="Projects heading"
        placeholder={`Projects from ${committee.name}`}
        maxLength={LIMITS.heading}
        onchange={(heading) => onchange({ heading })}
      />
      {#if projects.length > 0}
        <a
          class="projects__all"
          href={committeeProjectsPath(committee.slug)}
          tabindex={editor ? -1 : undefined}
        >
          See all on the Projects page
        </a>
      {/if}
    </div>
    {#if projects.length > 0}
      <ul class="projects__grid" inert={editor}>
        {#each projects as project (project.id)}
          <li class="projects__item">
            <ProjectCard {project} headingLevel={3} />
          </li>
        {/each}
      </ul>
    {:else}
      <p class="projects__empty">
        {committee.name}&rsquo;s published projects show here. Publish one under Projects in the portal and it
        appears on this page.
      </p>
    {/if}
  </section>
{/if}

<style>
  .projects {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .projects__heading {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px 24px;
  }
  .projects__all {
    color: var(--cb-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
    text-decoration: underline;
    text-underline-offset: 3px;

    &:hover {
      color: var(--cb-text);
    }
    &:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .projects__grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px 16px;
    margin: 0 -16px;
    padding: 0;
    list-style: none;
  }
  .projects__item {
    display: block;
    min-width: 0;
  }
  .projects__empty {
    margin: 0;
    padding: 20px;
    border: 1px dashed #b8b8b8;
    border-radius: 12px;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }

  @container committee-page (width < 900px) {
    .projects__grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
