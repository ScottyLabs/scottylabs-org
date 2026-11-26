<script lang="ts">
  // One project's page (Figma 10:217, 44:5275; phone 11:520; spec P-8 to P-12). Loaded in the
  // browser from /api/public/projects/<slug>. A renamed project's old address moves to the new
  // one; unpublished and archived projects show the not-found message. When the project cannot
  // load, the page shows it as it was on the last visit, with a note.
  import { tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import SearchX from "@lucide/svelte/icons/search-x";
  import { get, isAbortError, isApiError } from "#lib/api/client.ts";
  import type {
    ProjectCard,
    ProjectDetail,
    PublicProjectResponse,
    PublicProjectsResponse,
  } from "#lib/api/types.ts";
  import Skeleton from "#lib/portal/ui/Skeleton.svelte";
  import ProjectPage from "#lib/projects/ProjectPage.svelte";
  import LastGoodNote from "#lib/site/LastGoodNote.svelte";
  import { forget, isRecord, recall, remember } from "#lib/site/last-good.ts";
  import { projectsIndexHref, relatedProjects } from "#lib/projects/filter.ts";

  type View =
    | { kind: "loading" }
    /** `savedAt` is set when the project comes from the last visit's copy. */
    | { kind: "ready"; project: ProjectDetail; savedAt?: number }
    | { kind: "missing" }
    | { kind: "failed" };

  const SLUG = /^[a-z0-9]+(-[a-z0-9]+)*$/u;
  const isProject = (value: unknown): value is ProjectDetail =>
    isRecord(value) && typeof value.slug === "string" && typeof value.name === "string" && Array.isArray(value.media);
  const MOVED_TO = /^\/projects\/[a-z0-9]+(-[a-z0-9]+)*\/$/u;

  let view = $state<View>({ kind: "loading" });
  let related = $state.raw<ProjectCard[]>([]);
  let indexHref = $state("/projects/");
  let attempt = $state(0);

  const slug = $derived(page.params.slug ?? "");
  const title = $derived(
    view.kind === "ready"
      ? `${view.project.name} - ScottyLabs`
      : view.kind === "missing"
        ? "Project not found - ScottyLabs"
        : "Projects - ScottyLabs",
  );
  const description = $derived(
    view.kind === "ready"
      ? view.project.summary || view.project.excerpt || `${view.project.name}, a ScottyLabs project.`
      : "Apps, hackathons, brand work and ventures from all seven ScottyLabs committees.",
  );

  $effect(() => {
    void attempt;
    const controller = new AbortController();
    void load(slug, controller.signal);
    return () => controller.abort();
  });

  async function load(current: string, signal: AbortSignal): Promise<void> {
    indexHref = projectsIndexHref();
    view = { kind: "loading" };
    related = [];
    if (!SLUG.test(current)) {
      view = { kind: "missing" };
      return;
    }
    try {
      const response = await get<PublicProjectResponse>(`/public/projects/${current}`, { signal });
      if (response.redirectTo !== null && MOVED_TO.test(response.redirectTo)) {
        await goto(`${response.redirectTo}${page.url.hash}`, { replace: true });
        return;
      }
      if (response.project === null) {
        forget(`project:${current}`);
        view = { kind: "missing" };
        return;
      }
      view = { kind: "ready", project: response.project };
      remember(`project:${current}`, response.project);
      void revealTarget();
      void loadRelated(response.project, signal);
    } catch (error) {
      if (isAbortError(error)) return;
      if (isApiError(error) && error.status === 404) {
        forget(`project:${current}`);
        view = { kind: "missing" };
        return;
      }
      const saved = recall(`project:${current}`, isProject);
      view = saved === null ? { kind: "failed" } : { kind: "ready", project: saved.value, savedAt: saved.savedAt };
    }
  }

  /**
   * An address can point into the page, as the Projects page's "Watch video" does with #watch.
   * The browser looked for that part before the project loaded, so go to it now, and start
   * keyboard navigation there.
   */
  async function revealTarget(): Promise<void> {
    let id = "";
    try {
      id = decodeURIComponent(page.url.hash.slice(1));
    } catch {
      return;
    }
    if (id === "") return;
    await tick();
    const target = document.querySelector<HTMLElement>(`#${CSS.escape(id)}`);
    if (target === null) return;
    target.scrollIntoView({ block: "start" });
    if (!target.hasAttribute("tabindex")) target.tabIndex = -1;
    target.focus({ preventScroll: true });
  }

  async function loadRelated(project: ProjectDetail, signal: AbortSignal): Promise<void> {
    try {
      const response = await get<PublicProjectsResponse>("/public/projects", { signal });
      related = relatedProjects(response.projects, project);
    } catch {
      // Related projects are extra; the page works without them.
      related = [];
    }
  }
</script>

<svelte:head>
  <title>{title}</title>
  <meta name="description" content={description} />
</svelte:head>

<div class="project centered-section" aria-busy={view.kind === "loading"}>
  {#if view.kind === "ready"}
    {#if view.savedAt !== undefined}
      <LastGoodNote savedAt={view.savedAt} onretry={() => (attempt += 1)} />
    {/if}
    <ProjectPage project={view.project} {related} {indexHref} />
  {:else if view.kind === "loading"}
    <div class="skeleton" aria-hidden="true">
      <Skeleton width="180px" height="14px" />
      <div class="skeleton__showcase">
        <div class="skeleton__picture">
          <Skeleton width="100%" height="100%" radius="20px" />
        </div>
        <div class="skeleton__details">
          <Skeleton width="120px" height="20px" />
          <Skeleton width="70%" height="44px" />
          <Skeleton width="90%" height="22px" />
          <Skeleton width="100%" height="16px" />
          <Skeleton width="100%" height="16px" />
          <Skeleton width="60%" height="16px" />
          <Skeleton width="100%" height="48px" radius="100px" />
        </div>
      </div>
    </div>
    <p class="p-sr-only" role="status">Loading the project</p>
  {:else}
    <div class="message">
      <span class="message__icon" aria-hidden="true"><SearchX size={22} strokeWidth={1.75} /></span>
      {#if view.kind === "missing"}
        <h1 class="message__title">We couldn&rsquo;t find that project</h1>
        <p class="message__text">
          It may have moved or been taken down. The Projects page lists everything we&rsquo;re
          building now.
        </p>
        <a class="message__button" href={indexHref}>See all projects</a>
      {:else}
        <h1 class="message__title">We couldn&rsquo;t load this project</h1>
        <p class="message__text">Check your connection and try again.</p>
        <button type="button" class="message__button" onclick={() => (attempt += 1)}>Try again</button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .project {
    padding-top: 28px;
    padding-bottom: 96px;
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }

  .skeleton {
    container-type: inline-size;
  }
  .skeleton__showcase {
    display: grid;
    gap: 28px;
    margin-top: 24px;
  }
  .skeleton__picture {
    aspect-ratio: 4 / 3;
  }
  .skeleton__details {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  @container (min-width: 900px) {
    .skeleton__showcase {
      grid-template-columns: minmax(0, 2fr) minmax(300px, 1fr);
      gap: 48px;
    }
    .skeleton__picture {
      margin-left: 88px;
    }
  }

  .message {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    max-width: 560px;
    margin: 64px auto 0;
    padding: 56px 24px;
    border: 1px solid var(--p-border-muted);
    border-radius: 20px;
    text-align: center;
  }
  .message__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--p-surface-muted);
  }
  .message__title {
    margin: 0;
    font-size: 1.75rem;
    font-weight: 600;
    line-height: 36px;
    letter-spacing: -0.02em;
  }
  .message__text {
    max-width: 44ch;
    margin: 0 0 8px;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .message__button {
    display: inline-flex;
    align-items: center;
    min-height: 44px;
    box-sizing: border-box;
    padding: 12px 20px;
    border-radius: 100px;
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    cursor: pointer;

    &:hover {
      background: var(--p-inverse-hover);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
</style>
