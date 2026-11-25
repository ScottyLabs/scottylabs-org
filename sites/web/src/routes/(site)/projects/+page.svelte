<script lang="ts">
  // The Projects page (Figma 50:8319; phone 44:8364; spec P-1 to P-7). The page is prerendered
  // with a skeleton and loads the published projects in the browser. The committee tab and the
  // search text live in the address (?committee=&q=), so filtered lists can be shared and the
  // project pages' breadcrumb leads back to the same list. Searches filter the loaded cards at
  // once; from two characters the service also looks through whole descriptions, and what it finds
  // is added (filter.ts).
  import { onDestroy, tick, untrack } from "svelte";
  import { afterNavigate, goto } from "$app/navigation";
  import Plus from "@lucide/svelte/icons/plus";
  import SearchX from "@lucide/svelte/icons/search-x";
  import { get, isAbortError, isApiError } from "#lib/api/client.ts";
  import type { CommitteeSlug, ProjectCard as Card, PublicProjectsResponse } from "#lib/api/types.ts";
  import { COMMITTEES } from "#lib/portal/committees.ts";
  import Skeleton from "#lib/portal/ui/Skeleton.svelte";
  import CommitteeFilter from "#lib/projects/CommitteeFilter.svelte";
  import FeaturedProject from "#lib/projects/FeaturedProject.svelte";
  import ProjectCard from "#lib/projects/ProjectCard.svelte";
  import ProjectSearch from "#lib/projects/ProjectSearch.svelte";
  import {
    type ProjectFilters,
    type RemoteSearch,
    NO_FILTERS,
    awaitingRemote,
    committeeCounts,
    filterProjects,
    filtersFromParams,
    filtersQuery,
    pendingSearch,
    rememberFilters,
    remoteMatches,
    remoteQuery,
    resultsLine,
    searchKey,
    searchTextAfterNavigation,
  } from "#lib/projects/filter.ts";

  /** Cards shown before "Show more". */
  const PAGE_SIZE = 12;
  /** How long typing pauses before the address and the screen reader announcement catch up. */
  const SETTLE_MS = 400;
  /** How long typing pauses before the search goes to the service. */
  const REMOTE_DELAY_MS = 250;

  let projects = $state.raw<Card[] | null>(null);
  let loadError = $state<string | null>(null);
  let filters = $state<ProjectFilters>({ ...NO_FILTERS });
  let searchText = $state("");
  let showAll = $state(false);
  let announced = $state("");
  let grid: HTMLUListElement | undefined = $state();
  let settleTimer: ReturnType<typeof setTimeout> | undefined;
  /** The service's search over whole descriptions, for the current search text. */
  let remote = $state.raw<RemoteSearch | null>(null);
  let remoteTimer: ReturnType<typeof setTimeout> | undefined;
  let remoteRequest: AbortController | null = null;
  /** A settled search waits for the service before its count is announced. */
  let announceWhenAnswered = false;

  const alsoMatching = $derived(remoteMatches(remote, filters.q));
  const awaiting = $derived(awaitingRemote(remote, filters.q));
  const counts = $derived(projects === null ? null : committeeCounts(projects, filters.q, alsoMatching));
  const matching = $derived(projects === null ? [] : filterProjects(projects, filters, alsoMatching));
  // Nothing matches here yet, but the descriptions may hold a match: wait instead of saying none.
  const searching = $derived(projects !== null && projects.length > 0 && matching.length === 0 && awaiting);
  const featured = $derived(matching.find((project) => project.featured) ?? null);
  const rest = $derived(
    featured === null ? matching : matching.filter((project) => project.id !== featured.id),
  );
  const visible = $derived(showAll ? rest : rest.slice(0, PAGE_SIZE));
  const hiddenCount = $derived(rest.length - visible.length);
  const summary = $derived(resultsLine(matching.length, filters));

  // Coming here from a link (the header, a breadcrumb, a committee link) sets the filters from
  // the address. Changes made on the page update the address with a shallow navigation, which
  // runs this too; those already match the screen, and reading the trimmed search back would eat
  // the space or the letters typed since.
  afterNavigate(({ to, shallow }) => {
    if (to === null || shallow) return;
    const next = filtersFromParams(to.url.searchParams);
    filters = next;
    searchText = searchTextAfterNavigation(searchText, next.q);
    showAll = false;
    rememberFilters(next);
    if (projects === null && loadError === null) void load();
  });

  // A search still settling must not change the address of the page opened next.
  onDestroy(() => {
    clearTimeout(settleTimer);
    clearTimeout(remoteTimer);
    remoteRequest?.abort();
  });

  $effect(() => {
    const q = remoteQuery(filters.q);
    untrack(() => askService(q));
  });

  /** Sends the search to the service after a pause in typing, dropping any older one. */
  function askService(q: string | null): void {
    const key = q === null ? null : searchKey(q);
    // Asked already: the same words, typed differently or after a committee change.
    if (key !== null && remote?.key === key) return;
    clearTimeout(remoteTimer);
    remoteRequest?.abort();
    remoteRequest = null;
    if (q === null || key === null) {
      remote = null;
      return;
    }
    remote = pendingSearch(remote, key, projects ?? []);
    remoteTimer = setTimeout(() => void fetchMatches(q, key), REMOTE_DELAY_MS);
  }

  async function fetchMatches(q: string, key: string): Promise<void> {
    const request = new AbortController();
    remoteRequest = request;
    let answer: RemoteSearch;
    try {
      const response = await get<PublicProjectsResponse>("/public/projects", { query: { q }, signal: request.signal });
      answer = { key, status: "done", ids: new Set(response.projects.map((project) => project.id)) };
    } catch (error) {
      if (isAbortError(error)) return;
      // The loaded cards still filter; only matches deep in a description are missed.
      answer = { key, status: "failed" };
    } finally {
      if (remoteRequest === request) remoteRequest = null;
    }
    if (remote?.key !== key) return;
    remote = answer;
    if (announceWhenAnswered) announce();
  }

  /** Tells screen readers how many projects match, once the service has answered for the search. */
  function announce(): void {
    if (projects === null) {
      announced = "";
      return;
    }
    announceWhenAnswered = awaiting;
    if (!awaiting) announced = resultsLine(matching.length, filters);
  }

  async function load(): Promise<void> {
    loadError = null;
    try {
      const response = await get<PublicProjectsResponse>("/public/projects");
      projects = response.projects;
      announce();
    } catch (error) {
      loadError = isApiError(error)
        ? "We couldn\u2019t load the projects. Check your connection and try again."
        : "We couldn\u2019t load the projects. Try again in a moment.";
    }
  }

  function setFilters(next: ProjectFilters, settle: "now" | "later"): void {
    filters = next;
    showAll = false;
    rememberFilters(next);
    clearTimeout(settleTimer);
    announceWhenAnswered = false;
    const apply = () => {
      // Only the address changes; this is not a new page.
      void goto(`/projects/${filtersQuery(next)}`, { shallow: true, replace: true });
      announce();
    };
    if (settle === "now") apply();
    else settleTimer = setTimeout(apply, SETTLE_MS);
  }

  function selectCommittee(committee: CommitteeSlug | null): void {
    setFilters({ committee, q: filters.q }, "now");
  }

  function search(q: string): void {
    setFilters({ committee: filters.committee, q }, "later");
  }

  function reset(): void {
    searchText = "";
    setFilters({ ...NO_FILTERS }, "now");
  }

  async function showMore(): Promise<void> {
    const firstNew = visible.length;
    showAll = true;
    await tick();
    // Keyboard users continue from the first card that just appeared.
    grid?.querySelectorAll<HTMLElement>(".project-card__link")[firstNew]?.focus();
  }

  const tabHref = (committee: CommitteeSlug | null) =>
    `/projects/${filtersQuery({ committee, q: filters.q })}`;
</script>

<svelte:head>
  <title>Projects - ScottyLabs</title>
  <meta
    name="description"
    content="Apps, hackathons, brand work and ventures from all seven ScottyLabs committees."
  />
  <!-- Asked for from the HTML, so the projects and the featured picture do not wait for the app. -->
  <link rel="preload" as="fetch" crossorigin="anonymous" href="/api/public/projects" />
</svelte:head>

<div class="projects">
  <header class="hero centered-section">
    <h1 class="hero__title">
      Everything we&rsquo;re <span class="hero__code" aria-hidden="true">&lt;</span>building<span
        class="hero__code"
        aria-hidden="true">/&gt;</span
      >
    </h1>
    <p class="hero__intro">
      Apps, hackathons, brand work and ventures from all seven committees. Read a little about
      each, then open one for the full story.
    </p>
    <ProjectSearch bind:value={searchText} oninput={search} />
  </header>

  <div class="centered-section">
    <CommitteeFilter selected={filters.committee} {counts} href={tabHref} onselect={selectCommittee} />

    <div class="results" aria-busy={(projects === null && loadError === null) || searching}>
      <p class="results__line">
        {#if searching}
          Searching the descriptions&hellip;
        {:else if projects !== null}
          {summary}
        {:else if loadError === null}
          <Skeleton width="96px" height="14px" />
        {/if}
      </p>
      <p class="p-sr-only" aria-live="polite">{announced}</p>

      {#if loadError !== null}
        <div class="notice" role="alert">
          <p class="notice__text">{loadError}</p>
          <button type="button" class="pill pill--outline" onclick={() => void load()}>Try again</button>
        </div>
      {:else if projects === null}
        <div class="skeleton-featured" aria-hidden="true">
          <Skeleton width="100%" height="100%" radius="20px" class="skeleton-featured__cover" />
          <div class="skeleton-featured__text">
            <Skeleton width="60%" height="44px" />
            <Skeleton width="100%" height="16px" />
            <Skeleton width="85%" height="16px" />
          </div>
        </div>
        <ul class="grid" role="list" aria-hidden="true">
          {#each [0, 1, 2, 3] as item (item)}
            <li class="skeleton-card">
              <Skeleton width="100%" height="100%" radius="14px" class="skeleton-card__cover" />
              <div class="skeleton-card__text">
                <Skeleton width="50%" height="20px" />
                <Skeleton width="100%" height="14px" />
                <Skeleton width="80%" height="14px" />
              </div>
            </li>
          {/each}
        </ul>
      {:else if searching}
        <!-- No card matches yet; the service is still reading the descriptions. -->
      {:else if matching.length === 0}
        <div class="empty">
          <span class="empty__icon" aria-hidden="true"><SearchX size={22} strokeWidth={1.75} /></span>
          {#if projects.length === 0}
            <h2 class="empty__title">No projects are published yet</h2>
            <p class="empty__text">Check back soon, or add yours below.</p>
          {:else if filters.q.trim() !== ""}
            <h2 class="empty__title">No projects match &ldquo;{filters.q.trim()}&rdquo;</h2>
            <p class="empty__text">
              Check the spelling, or try the name of a committee or a word such as app or event.
            </p>
            <button type="button" class="pill pill--outline" onclick={reset}>Show all projects</button>
          {:else if filters.committee !== null}
            <h2 class="empty__title">{COMMITTEES[filters.committee].name} has no projects here yet</h2>
            <p class="empty__text">See what the other committees are building.</p>
            <button type="button" class="pill pill--outline" onclick={reset}>Show all projects</button>
          {/if}
        </div>
      {:else}
        {#if featured !== null}
          <FeaturedProject project={featured} />
        {/if}
        {#if visible.length > 0}
          <h2 class="p-sr-only">{featured === null ? "Projects" : "More projects"}</h2>
          <ul class={["grid", featured !== null && "grid--after-featured"]} role="list" bind:this={grid}>
            {#each visible as project (project.id)}
              <!-- Below the featured project, the cards start under the fold on every screen. -->
              <li><ProjectCard {project} headingLevel={3} /></li>
            {/each}
          </ul>
        {/if}
        {#if hiddenCount > 0}
          <div class="more">
            <button type="button" class="pill pill--outline pill--large" onclick={() => void showMore()}>
              Show {hiddenCount} more {hiddenCount === 1 ? "project" : "projects"}
            </button>
          </div>
        {/if}
      {/if}
    </div>
  </div>

  <section class="invite" aria-labelledby="invite-title">
    <h2 class="invite__title" id="invite-title">Working on something at ScottyLabs?</h2>
    <p class="invite__text">
      Sign in with your ScottyLabs account to add your project: screenshots, videos, a write-up and
      the repo link.
    </p>
    <a class="pill pill--filled pill--large" href="/portal/projects/">
      <Plus size={20} strokeWidth={2} aria-hidden="true" />
      Add your project
    </a>
  </section>
</div>

<style>
  .projects {
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }

  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    padding-top: 88px;
    padding-bottom: 44px;
    text-align: center;
  }
  .hero__title {
    margin: 0;
    font-size: clamp(2.5rem, 6vw, 3.5rem);
    font-weight: 600;
    line-height: 1.1;
    letter-spacing: -0.045em;
  }
  .hero__code {
    color: var(--black-300);
    font-family: JetBrainsMono, var(--p-mono);
    font-weight: 400;
    letter-spacing: -0.06em;
  }
  .hero__intro {
    max-width: 640px;
    margin: 0 0 12px;
    color: var(--p-text-secondary);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }

  .results {
    padding-top: 32px;
  }
  .results__line {
    display: flex;
    align-items: center;
    min-height: 20px;
    margin: 0 0 28px;
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
    margin: 0 -16px;
    padding: 0;
  }
  .grid li {
    display: block;
    min-width: 0;
  }
  .grid--after-featured {
    margin-top: 48px;
  }

  .skeleton-featured {
    display: grid;
    grid-template-columns: minmax(0, 520fr) minmax(0, 632fr);
    gap: 48px;
  }
  .skeleton-featured :global(.skeleton-featured__cover) {
    aspect-ratio: 16 / 10;
  }
  .skeleton-featured__text,
  .skeleton-card__text {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .skeleton-card {
    display: grid;
    grid-template-columns: minmax(140px, 236px) minmax(0, 1fr);
    gap: 24px;
    padding: 16px;
  }
  .skeleton-card :global(.skeleton-card__cover) {
    aspect-ratio: 4 / 3;
  }
  .skeleton-featured + .grid {
    margin-top: 48px;
  }

  .notice,
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 56px 20px;
    border: 1px solid var(--p-border-muted);
    border-radius: 20px;
    text-align: center;
  }
  .notice__text,
  .empty__text {
    max-width: 46ch;
    margin: 0 0 8px;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .empty__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--p-surface-muted);
  }
  .empty__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    line-height: 28px;
    overflow-wrap: anywhere;
  }

  .more {
    display: flex;
    justify-content: center;
    padding: 56px 0;
  }

  .pill {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    box-sizing: border-box;
    min-height: 38px;
    padding: 9px 16px;
    border-radius: 100px;
    font-family: var(--p-font);
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    cursor: pointer;
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease);

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .pill--large {
    min-height: 48px;
    padding: 14px 20px;
  }
  .pill--outline {
    border: 1px solid var(--p-border);
    background: var(--p-surface);
    color: var(--p-text);

    &:hover {
      border-color: var(--p-text-secondary);
    }
  }
  .pill--filled {
    background: var(--p-inverse);
    color: var(--p-on-inverse);

    &:hover {
      background: var(--p-inverse-hover);
    }
  }

  .invite {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    margin-top: 24px;
    padding: 72px 20px;
    background: var(--p-surface-muted);
    text-align: center;
  }
  .invite__title {
    margin: 0;
    font-size: 1.875rem;
    font-weight: 600;
    line-height: 38px;
  }
  .invite__text {
    max-width: 560px;
    margin: 0 0 12px;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }

  @media (max-width: 900px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
      margin: 0;
    }
    .skeleton-featured {
      grid-template-columns: minmax(0, 1fr);
      gap: 20px;
    }
  }
  @media (max-width: 760px) {
    .hero {
      padding-top: 40px;
      padding-bottom: 28px;
    }
    .hero__intro {
      font-size: 1rem;
      line-height: 24px;
    }
    .grid--after-featured {
      margin-top: 12px;
    }
    .grid {
      gap: 12px;
    }
    .skeleton-card {
      grid-template-columns: 96px minmax(0, 1fr);
      gap: 12px;
      padding: 14px;
      border: 1px solid var(--p-border-muted);
      border-radius: var(--p-radius-xl);
    }
    .skeleton-card :global(.skeleton-card__cover) {
      aspect-ratio: 1;
    }
    .invite {
      padding: 48px 20px;
    }
    .invite__title {
      font-size: 1.5rem;
      line-height: 32px;
    }
    .invite .pill {
      width: 100%;
      max-width: 400px;
    }
  }
</style>
