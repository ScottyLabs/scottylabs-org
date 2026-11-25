<script lang="ts">
  // The featured project at the top of the Projects page (Figma 52:8705): a large cover, the
  // name and description, its facts and links. Narrow screens show the usual card instead.
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import type { ProjectCard as Card } from "../api/types.ts";
  import Img from "../media/Img.svelte";
  import { COMMITTEES } from "../portal/committees.ts";
  import CommitteeChip from "../portal/ui/CommitteeChip.svelte";
  import SelectionBox from "../site/SelectionBox.svelte";
  import ProjectCard from "./ProjectCard.svelte";
  import { linkHost } from "../site/links.ts";
  import { PROJECT_KIND_LABELS, PROJECT_STATUS_LABELS, projectPath } from "./labels.ts";

  let { project }: { project: Card } = $props();

  const href = $derived(projectPath(project.slug));
  const style = $derived(COMMITTEES[project.committee]);
  const description = $derived(project.excerpt.trim() === "" ? project.summary : project.excerpt);
</script>

<section class="featured" aria-labelledby="featured-label">
  <p class="featured__label" id="featured-label">Featured</p>

  <article class="featured__wide sl-select-host">
    <div class="featured__cover" style:--tint={style.tint}>
      {#if project.cover !== null}
        <Img
          image={project.cover}
          alt=""
          sizes="(min-width: 1200px) 520px, 45vw"
          fallbackWidth={960}
          fit="cover"
          loading="eager"
          fetchpriority="high"
        />
      {:else}
        <span class="featured__monogram" aria-hidden="true">{project.name}</span>
      {/if}
    </div>
    <div class="featured__content">
      <div class="featured__text">
        <h2 class="featured__name">
          <a class="featured__link" {href} data-selection-target>{project.name}</a>
        </h2>
        <p class="featured__description">{description}</p>
      </div>
      <div class="featured__bottom">
        <div class="featured__facts">
          <CommitteeChip committee={project.committee} size="sm" />
          <span class="featured__status">
            <span
              class={["featured__status-dot", project.status === "live" && "featured__status-dot--live"]}
              aria-hidden="true"
            ></span>
            {PROJECT_STATUS_LABELS[project.status]}
          </span>
          <span class="featured__divider" aria-hidden="true"></span>
          <span class="featured__kind">{PROJECT_KIND_LABELS[project.kind]}</span>
        </div>
        <div class="featured__actions">
          <!-- The card's own link covers this; it only shows where to click. -->
          <span class="featured__button featured__button--filled" aria-hidden="true">View project</span>
          {#if project.siteUrl !== null}
            <a
              class="featured__button featured__button--outline"
              href={project.siteUrl}
              target="_blank"
              rel="noopener noreferrer"
            >
              {linkHost(project.siteUrl) ?? "Website"}
              <ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
              <span class="p-sr-only">(opens in a new tab)</span>
            </a>
          {/if}
        </div>
      </div>
    </div>
    <SelectionBox label={`/projects/${project.slug}`} />
  </article>

  <div class="featured__narrow">
    <ProjectCard {project} headingLevel={2} loading="eager" />
  </div>
</section>

<style>
  .featured {
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }
  .featured__label {
    display: inline-block;
    margin: 0 0 16px;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--p-surface-muted);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .featured__wide {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 520fr) minmax(0, 632fr);
    gap: 48px;
  }
  .featured__cover {
    position: relative;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
    aspect-ratio: 16 / 10;
    border-radius: 20px;
    background: var(--tint);

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px var(--p-border-muted);
    }
  }
  .featured__monogram {
    padding: 24px;
    font-size: 3rem;
    font-weight: 600;
    letter-spacing: -0.04em;
  }
  .featured__content {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 20px;
    min-width: 0;
  }
  .featured__text {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .featured__name {
    margin: 0;
    font-size: 2.75rem;
    font-weight: 600;
    line-height: 50px;
    letter-spacing: -0.045em;
  }
  .featured__link {
    color: inherit;

    /* The whole card opens the project; the site link sits above this layer. */
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
  @supports selector(:has(a)) {
    .featured__link:focus-visible {
      outline: none;
    }
  }
  .featured__description {
    max-width: 560px;
    margin: 0;
    font-size: 1rem;
    line-height: 24px;
  }
  .featured__bottom {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .featured__facts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    font-size: 0.75rem;
    line-height: 16px;
  }
  .featured__status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--p-text-secondary);
  }
  .featured__status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--p-neutral);
  }
  .featured__status-dot--live {
    background: #0a722e;
  }
  .featured__divider {
    width: 1px;
    height: 12px;
    background: var(--p-border-muted);
  }
  .featured__kind {
    color: var(--p-text-tertiary);
  }
  .featured__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .featured__button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 38px;
    box-sizing: border-box;
    padding: 9px 16px;
    border-radius: 100px;
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease);

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .featured__button--filled {
    background: var(--p-inverse);
    color: var(--p-on-inverse);
  }
  .featured__wide:hover .featured__button--filled {
    background: var(--p-inverse-hover);
  }
  .featured__button--outline {
    position: relative;
    z-index: 2;
    border: 1px solid var(--p-border);
    background: var(--p-surface);
    color: var(--p-text);

    &:hover {
      border-color: var(--p-text-secondary);
    }
  }
  .featured__narrow {
    display: none;
  }

  @media (max-width: 760px) {
    .featured__label {
      display: none;
    }
    .featured__wide {
      display: none;
    }
    .featured__narrow {
      display: block;
    }
  }
</style>
