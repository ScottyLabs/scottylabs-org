<script lang="ts">
  // A small square card for "Related projects" on a project page (Figma 10:425): the cover, the
  // name with its committee, and the one-line summary. The whole tile is one link.
  import type { ProjectCard } from "../api/types.ts";
  import Img from "../media/Img.svelte";
  import { COMMITTEES } from "../portal/committees.ts";
  import { projectPath } from "./labels.ts";

  let { project }: { project: ProjectCard } = $props();

  const style = $derived(COMMITTEES[project.committee]);
</script>

<article class="tile" style:--tint={style.tint} style:--dot={style.color}>
  <div class="tile__cover">
    {#if project.cover !== null}
      <Img image={project.cover} alt="" sizes="(min-width: 900px) 282px, 50vw" fallbackWidth={480} fit="cover" />
    {:else}
      <span class="tile__cover-name" aria-hidden="true">{project.name}</span>
    {/if}
  </div>
  <div class="tile__title-row">
    <h3 class="tile__name">
      <a class="tile__link" href={projectPath(project.slug)}>{project.name}</a>
    </h3>
    <span class="tile__committee">
      <span class="tile__dot" aria-hidden="true"></span>
      {style.name}
    </span>
  </div>
  {#if project.summary !== ""}
    <p class="tile__summary">{project.summary}</p>
  {/if}
</article>

<style>
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .tile__cover {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
    margin-bottom: 8px;
    aspect-ratio: 1;
    border-radius: 20px;
    background: var(--tint);
    transition: transform var(--p-duration) var(--p-ease);

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px var(--p-border);
    }
  }
  .tile:hover .tile__cover {
    transform: translateY(-2px);
  }
  .tile__cover-name {
    padding: 22px;
    font-size: 1.875rem;
    font-weight: 600;
    line-height: 34px;
    letter-spacing: -0.05em;
    overflow-wrap: anywhere;
  }
  .tile__title-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }
  .tile__name {
    overflow: hidden;
    margin: 0;
    font-size: 1.125rem;
    font-weight: 500;
    line-height: 26px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tile__link {
    color: inherit;

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: 20px;
    }
    &:focus-visible {
      outline: none;
    }
    &:focus-visible::after {
      outline: 2px solid var(--p-focus);
      outline-offset: 4px;
    }
  }
  .tile__committee {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .tile__dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--dot);
  }
  .tile__summary {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  @media (prefers-reduced-motion: reduce) {
    .tile:hover .tile__cover {
      transform: none;
    }
  }
</style>
