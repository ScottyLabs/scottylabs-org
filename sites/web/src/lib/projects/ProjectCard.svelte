<script lang="ts" module>
  import type { ProjectKind, ProjectStatus } from "../api/types.ts";

  export const PROJECT_STATUS_LABELS: Readonly<Record<ProjectStatus, string>> = {
    live: "Live",
    in_development: "In development",
    annual: "Annual",
    ongoing: "Ongoing",
  };

  export const PROJECT_KIND_LABELS: Readonly<Record<ProjectKind, string>> = {
    app: "App",
    event: "Event",
    tool: "Tool",
    brand: "Brand",
    program: "Program",
    fundraiser: "Fundraiser",
    other: "Other",
  };

  /** The public page of a project, with the trailing slash the site serves it under. */
  export function projectPath(slug: string): string {
    return `/projects/${encodeURIComponent(slug)}/`;
  }
</script>

<script lang="ts">
  // A project on the Projects page, a committee page and previews in the portal: the cover, name,
  // excerpt and facts, with repository and site links. The whole card opens the project; the
  // links are separate tab stops above it. Below 520 px of width it switches to the phone layout.
  //   <ProjectCard project={card} headingLevel={2} />
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import type { ProjectCard } from "../api/types.ts";
  import Img from "../media/Img.svelte";
  import { COMMITTEES } from "../portal/committees.ts";
  import CommitteeChip from "../portal/ui/CommitteeChip.svelte";
  import SelectionBox from "../site/SelectionBox.svelte";

  let {
    project,
    headingLevel = 3,
    href: hrefProp,
    loading = "lazy",
    class: className,
  }: {
    project: ProjectCard;
    /** The heading level of the project name in the page outline. */
    headingLevel?: 2 | 3 | 4 | 5 | 6;
    /** Where the card leads; the project page by default. `null` shows it without a link, for previews of drafts. */
    href?: string | null;
    /** Use "eager" for cards at the top of the page. */
    loading?: "lazy" | "eager";
    class?: string;
  } = $props();

  const committee = $derived(COMMITTEES[project.committee]);
  const href = $derived(hrefProp === undefined ? projectPath(project.slug) : hrefProp);
  const monogram = $derived(firstLetter(project.name));

  function firstLetter(name: string): string {
    const code = name.trim().codePointAt(0);
    return code === undefined ? "" : String.fromCodePoint(code).toUpperCase();
  }
  const description = $derived(project.excerpt.trim() === "" ? project.summary : project.excerpt);
</script>

<article
  class={["project-card", "sl-select-host", className]}
  style:--card-color={committee.color}
  style:--card-tint={committee.tint}
>
  <div class="project-card__frame">
    <div class={["project-card__cover", project.cover === null && "project-card__cover--empty"]}>
      {#if project.cover !== null}
        <Img
          image={project.cover}
          alt=""
          sizes="(max-width: 560px) 96px, 236px"
          fallbackWidth={480}
          fit="cover"
          {loading}
        />
      {:else}
        <span class="project-card__monogram" aria-hidden="true">{monogram}</span>
        <span class="project-card__cover-name" aria-hidden="true">{project.name}</span>
      {/if}
    </div>

    <div class="project-card__body">
      <svelte:element this={`h${headingLevel.toString()}`} class="project-card__title">
        {#if href === null}
          {project.name}
        {:else}
          <a class="project-card__link" {href} data-selection-target>{project.name}</a>
        {/if}
      </svelte:element>

      <span class="project-card__committee">
        <span class="project-card__committee-dot" aria-hidden="true"></span>
        {committee.name}
      </span>

      {#if project.repoUrl !== null || project.siteUrl !== null}
        <div class="project-card__links">
          {#if project.repoUrl !== null}
            <a
              class="project-card__icon-link"
              href={project.repoUrl}
              target="_blank"
              rel="noopener noreferrer"
              title="Repository"
              aria-label={`${project.name} repository (opens in a new tab)`}
            >
              <GitBranch size={16} strokeWidth={1.75} aria-hidden="true" />
            </a>
          {/if}
          {#if project.siteUrl !== null}
            <a
              class="project-card__icon-link"
              href={project.siteUrl}
              target="_blank"
              rel="noopener noreferrer"
              title="Website"
              aria-label={`${project.name} website (opens in a new tab)`}
            >
              <ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" />
            </a>
          {/if}
        </div>
      {/if}

      <p class="project-card__description">{description}</p>

      <div class="project-card__facts">
        <span class="project-card__chip"><CommitteeChip committee={project.committee} size="sm" /></span>
        <span class="project-card__status">
          <span
            class={["project-card__status-dot", project.status === "live" && "project-card__status-dot--live"]}
            aria-hidden="true"
          ></span>
          {PROJECT_STATUS_LABELS[project.status]}
        </span>
        <span class="project-card__divider" aria-hidden="true"></span>
        <span class="project-card__kind">{PROJECT_KIND_LABELS[project.kind]}</span>
      </div>
    </div>
  </div>
  {#if href !== null}
    <SelectionBox label={`/projects/${project.slug}`} />
  {/if}
</article>

<style>
  .project-card {
    position: relative;
    container-type: inline-size;
    min-width: 0;
    color: var(--p-text);
    font-family: var(--p-font);
  }

  /* Desktop (Figma 50:8317): cover on the left, text on the right, no box until hovered. */
  .project-card__frame {
    display: grid;
    grid-template-columns: minmax(140px, 236px) minmax(0, 1fr);
    gap: 24px;
    padding: 16px;
  }
  .project-card__cover {
    position: relative;
    overflow: hidden;
    aspect-ratio: 4 / 3;
    border-radius: var(--p-radius-lg);
    background: var(--p-surface-strong);

    /* The quiet inner stroke, drawn over the picture. */
    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
      pointer-events: none;
    }
  }
  .project-card__cover--empty {
    display: flex;
    align-items: flex-end;
    background: var(--card-tint);
  }
  .project-card__monogram {
    padding: 0 16px 8px;
    color: var(--card-color);
    font-size: 56px;
    font-weight: 700;
    line-height: 62px;
    letter-spacing: -0.025em;
  }
  .project-card__cover-name {
    display: none;
  }

  .project-card__body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: auto auto 1fr;
    grid-template-areas:
      "title links"
      "description description"
      "facts facts";
    align-items: start;
    column-gap: 8px;
    min-width: 0;
  }
  .project-card__title {
    grid-area: title;
    overflow: hidden;
    margin: 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 700;
    line-height: 28px;
    letter-spacing: -0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .project-card__link {
    color: inherit;
    text-decoration: none;

    /* Stretched over the whole card, so the card is one link without nesting the icon links. */
    &::after {
      content: "";
      position: absolute;
      inset: 0;
      z-index: 1;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  /* The selection box marks keyboard focus where :has() works. */
  @supports selector(:has(a)) {
    .project-card__link:focus-visible {
      outline: none;
    }
  }
  .project-card__committee {
    display: none;
  }
  .project-card__links {
    grid-area: links;
    position: relative;
    z-index: 2;
    display: flex;
    gap: 2px;
  }
  .project-card__icon-link {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text);
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover {
      background: var(--p-surface-strong);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .project-card__description {
    grid-area: description;
    display: -webkit-box;
    overflow: hidden;
    margin: 6px 0 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 400;
    line-height: 20px;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
  }
  .project-card__facts {
    grid-area: facts;
    align-self: end;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 10px;
    margin-top: 12px;
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
  }
  .project-card__chip {
    display: inline-flex;
  }
  .project-card__status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--p-text-secondary);
  }
  .project-card__status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--p-neutral);
  }
  .project-card__status-dot--live {
    background: #0a722e;
  }
  .project-card__divider {
    width: 1px;
    height: 12px;
    background: var(--p-border-muted);
  }
  .project-card__kind {
    color: var(--p-text-tertiary);
  }

  /* Phone (Figma 44:8364): a bordered card with a square picture and the committee by the name. */
  @container (max-width: 519px) {
    .project-card__frame {
      grid-template-columns: 96px minmax(0, 1fr);
      gap: 12px;
      padding: 14px;
      border: 1px solid var(--p-border);
      border-radius: var(--p-radius-xl);
      background: var(--p-surface);
    }
    .project-card__cover {
      align-self: start;
      aspect-ratio: 1;
      border-radius: var(--p-radius);

      &::after {
        box-shadow: inset 0 0 0 1px var(--p-border);
      }
    }
    .project-card__monogram {
      display: none;
    }
    .project-card__cover-name {
      display: block;
      overflow: hidden;
      padding: 8px;
      color: var(--p-text);
      font-size: 0.75rem;
      font-weight: 500;
      line-height: 16px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .project-card__body {
      grid-template-areas:
        "title committee"
        "description description"
        "facts links";
    }
    .project-card__title {
      font-size: 1rem;
      font-weight: 500;
      line-height: 24px;
      letter-spacing: 0;
    }
    .project-card__committee {
      grid-area: committee;
      display: inline-flex;
      align-items: center;
      gap: 6px;
      margin-top: 4px;
      color: var(--p-text-secondary);
      font-size: 0.75rem;
      font-weight: 400;
      line-height: 16px;
      white-space: nowrap;
    }
    .project-card__committee-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--card-color);
    }
    .project-card__description {
      margin-top: 4px;
    }
    .project-card__links {
      align-self: end;
      margin-top: 6px;
    }
    .project-card__facts {
      margin-top: 6px;
      min-height: 28px;
    }
    /* The committee sits by the name instead. */
    .project-card__chip {
      display: none;
    }
  }
</style>
