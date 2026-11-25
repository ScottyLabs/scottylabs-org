<script lang="ts">
  // One project as visitors see it (Figma 10:217, 44:5275; phone 11:520): the photos, name,
  // summary and description, links, facts with links to committee pages, the video and related
  // projects. The portal shows drafts with it too. It lays itself out by its own width.
  //   <ProjectPage project={detail} related={cards} indexHref="/projects/?committee=tech" />
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import type { ProjectCard, ProjectDetail } from "../api/types.ts";
  import RichText from "../editor/RichText.svelte";
  import { COMMITTEES } from "../portal/committees.ts";
  import ProjectGallery from "./ProjectGallery.svelte";
  import ProjectTile from "./ProjectTile.svelte";
  import VideoPlayer from "./VideoPlayer.svelte";
  import { linkHost } from "../site/links.ts";
  import { PROJECT_KIND_LABELS, PROJECT_STATUS_LABELS, linkLabel } from "./labels.ts";

  let {
    project,
    related = [],
    indexHref = "/projects/",
    preview = false,
  }: {
    project: ProjectDetail;
    /** Other projects to suggest at the bottom; none hides the section. */
    related?: readonly ProjectCard[];
    /** Where the "Projects" crumb leads, with the visitor's last filters. */
    indexHref?: string;
    /**
     * A draft shown in the portal, under the portal page's own h1: the name is an h2, and the
     * links to other public pages, which show only what is published, become plain text.
     */
    preview?: boolean;
  } = $props();

  const committee = $derived(COMMITTEES[project.committee]);
  const committeeProjects = $derived(`/projects/?committee=${project.committee}`);
  const hasDescription = $derived(project.description.trim() !== "");
  const nameTag = $derived(preview ? "h2" : "h1");
  const sectionTag = $derived(preview ? "h3" : "h2");
  /** A link to a public page, or no address in a preview. */
  const publicHref = (href: string) => (preview ? undefined : href);
</script>

{#snippet committeeLink(slug: ProjectDetail["committee"])}
  <a class="facts__link" href={publicHref(`/committees/${slug}/`)}>{COMMITTEES[slug].name}</a>
{/snippet}

<div class="project-page-host">
  <article class="project-page" style:--committee-color={committee.color}>
    <nav class="crumbs" aria-label="Breadcrumb">
      <ol>
        <li><a href={publicHref(indexHref)}>Projects</a></li>
        <li><a href={publicHref(committeeProjects)}>{committee.name}</a></li>
        <li><span aria-current="page">{project.name}</span></li>
      </ol>
    </nav>

    <div class="showcase">
      <ProjectGallery images={project.media} name={project.name} committee={project.committee} />

      <div class="details">
        <div class="details__tags">
          <span class="details__committee">
            <span class="details__dot" aria-hidden="true"></span>
            {committee.name}
          </span>
          <span class="details__tag">{PROJECT_STATUS_LABELS[project.status]}</span>
          <span class="details__tag">{PROJECT_KIND_LABELS[project.kind]}</span>
        </div>

        <div class="details__title">
          <svelte:element this={nameTag} class="details__name">{project.name}</svelte:element>
          {#if project.summary !== ""}
            <p class="details__summary">{project.summary}</p>
          {/if}
        </div>

        {#if hasDescription}
          <RichText html={project.description} class="details__description" />
        {/if}

        {#if project.siteUrl !== null || project.repoUrl !== null}
          <div class="details__links">
            {#if project.siteUrl !== null}
              <a class="site-link" href={project.siteUrl} target="_blank" rel="noopener noreferrer">
                Visit {linkHost(project.siteUrl) ?? "the website"}
                <ArrowUpRight size={20} strokeWidth={2} aria-hidden="true" />
                <span class="p-sr-only">(opens in a new tab)</span>
              </a>
            {/if}
            {#if project.repoUrl !== null}
              <a class="repo-link" href={project.repoUrl} target="_blank" rel="noopener noreferrer">
                <GitBranch size={20} strokeWidth={1.75} aria-hidden="true" />
                <span class="repo-link__text">
                  <span class="repo-link__label">Repository</span>
                  <span class="repo-link__url">{linkLabel(project.repoUrl)}</span>
                </span>
                <ExternalLink size={18} strokeWidth={1.75} aria-hidden="true" />
                <span class="p-sr-only">(opens in a new tab)</span>
              </a>
            {/if}
          </div>
        {/if}

        <dl class="facts">
          <div class="facts__row">
            <dt>Committee</dt>
            <dd>{@render committeeLink(project.committee)}</dd>
          </div>
          {#if project.builtWith.length > 0}
            <div class="facts__row">
              <dt>Built with</dt>
              <dd>
                {#each project.builtWith as slug, position (slug)}{position > 0 ? ", " : ""}{@render committeeLink(slug)}{/each}
              </dd>
            </div>
          {/if}
          <div class="facts__row">
            <dt>Status</dt>
            <dd>{PROJECT_STATUS_LABELS[project.status]}</dd>
          </div>
          <div class="facts__row">
            <dt>Type</dt>
            <dd>{PROJECT_KIND_LABELS[project.kind]}</dd>
          </div>
        </dl>
      </div>
    </div>

    {#if project.videoUrl !== null}
      <section class="section" id="watch" aria-labelledby="watch-heading">
        <svelte:element this={sectionTag} class="section__heading" id="watch-heading">Watch</svelte:element>
        <div class="section__video">
          <VideoPlayer url={project.videoUrl} title={`${project.name} video`} poster={project.cover} />
        </div>
      </section>
    {/if}

    {#if related.length > 0}
      <section class="section" aria-labelledby="related-heading">
        <div class="section__head">
          <svelte:element this={sectionTag} class="section__heading" id="related-heading">
            Related projects
          </svelte:element>
          <a class="section__more" href={publicHref(committeeProjects)}>
            All {committee.name} projects
            <ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
          </a>
        </div>
        <ul class="related" role="list">
          {#each related as card (card.id)}
            <li><ProjectTile project={card} /></li>
          {/each}
        </ul>
      </section>
    {/if}
  </article>
</div>

<style>
  .project-page-host {
    container-type: inline-size;
    min-width: 0;
  }
  .project-page {
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }

  .crumbs ol {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.875rem;
    line-height: 20px;
  }
  .crumbs li {
    display: inline-flex;
    gap: 8px;
    color: var(--p-text-secondary);

    &:not(:last-child)::after {
      content: "/";
      color: var(--p-text-tertiary);
    }
  }
  .crumbs a {
    color: var(--p-text-secondary);
    text-decoration: none;
    border-radius: 4px;

    &[href]:hover {
      color: var(--p-text);
      text-decoration: underline;
      text-underline-offset: 3px;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .crumbs [aria-current="page"] {
    color: var(--p-text);
  }

  .showcase {
    display: grid;
    gap: 28px;
    margin-top: 20px;
  }
  .details {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .details__tags {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .details__committee {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .details__dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--committee-color);
  }
  .details__tag {
    padding: 3px 7px;
    border: 1px solid var(--p-border);
    border-radius: 4px;
    background: var(--p-surface);
  }
  .details__title {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .details__name {
    margin: 0;
    font-size: clamp(2.25rem, 7cqi, 2.75rem);
    font-weight: 600;
    line-height: 1.14;
    letter-spacing: -0.045em;
    overflow-wrap: anywhere;
  }
  .details__summary {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .details :global(.details__description) {
    font-size: 1rem;
    line-height: 24px;
  }
  .details :global(.details__description a) {
    color: var(--p-text);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .details__links {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 4px;
  }
  .site-link {
    /* The public site has no global border-box rule; without it the padding adds to the 48 px. */
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 48px;
    padding: 12px 20px;
    border-radius: 100px;
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    text-align: center;
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover {
      background: var(--p-inverse-hover);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .repo-link {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--p-border);
    border-radius: 12px;
    color: var(--p-text);
    transition: border-color var(--p-duration) var(--p-ease);

    &:hover {
      border-color: var(--p-text-secondary);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .repo-link__text {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  .repo-link__label {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .repo-link__url {
    overflow: hidden;
    font-family: var(--p-mono);
    font-size: 0.875rem;
    line-height: 20px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .facts {
    margin: 0;
    font-size: 0.875rem;
    line-height: 20px;
  }
  .facts__row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 11px 0;
    border-top: 1px solid var(--p-border-muted);

    &:last-child {
      border-bottom: 1px solid var(--p-border-muted);
    }
  }
  .facts dt {
    flex: none;
    color: var(--p-text-secondary);
  }
  .facts dd {
    margin: 0;
    text-align: right;
  }
  .facts__link {
    color: var(--p-text);
    text-decoration: underline;
    text-decoration-color: var(--p-border);
    text-underline-offset: 3px;

    &[href]:hover {
      text-decoration-color: currentColor;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
      border-radius: 2px;
    }
  }

  .section {
    margin-top: 56px;
  }
  .section__head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
  }
  .section__heading {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 500;
    line-height: 32px;
  }
  .section__video {
    margin-top: 16px;
  }
  .section__more {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 100px;
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .related {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 24px 16px;
    margin: 24px 0 0;
    padding: 0;
    list-style: none;
  }
  .related li {
    display: block;
    min-width: 0;
  }

  @container (min-width: 900px) {
    .showcase {
      grid-template-columns: minmax(0, 2fr) minmax(300px, 1fr);
      gap: 48px;
      margin-top: 24px;
    }
    .section {
      margin-top: 80px;
    }
    .section__video {
      max-width: calc((100% - 48px) * 2 / 3);
    }
    .related {
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 24px;
    }
  }
</style>
