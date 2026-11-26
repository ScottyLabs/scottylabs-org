<script lang="ts">
  // The Committees index (spec C-1 to C-5): an introduction with jump chips, one row per committee
  // with its picture alternating sides, and the Saturday work sessions with links to CMU Maps.
  // When the committees cannot load, the page shows what loaded before, in this visit or the last
  // one, with a note, and an error only when it has nothing.
  import "#lib/committees/committees.css";
  import MapPin from "@lucide/svelte/icons/map-pin";
  import Users from "@lucide/svelte/icons/users";
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { CommitteeSlug, CommitteeSummary } from "#lib/api/types.ts";
  import { loadPublicCommittees } from "#lib/committees/api.ts";
  import { JOIN_URL, committeePath } from "#lib/committees/links.ts";
  import Img from "#lib/media/Img.svelte";
  import { COMMITTEE_LIST, committeeNames } from "#lib/portal/committees.ts";
  import Skeleton from "#lib/portal/ui/Skeleton.svelte";
  import { SelectionBox } from "#lib/site/index.ts";
  import LastGoodNote from "#lib/site/LastGoodNote.svelte";
  import { isRecord, recall, remember } from "#lib/site/last-good.ts";

  const SAVED_KEY = "committees";

  let committees = $state.raw<CommitteeSummary[] | null>(null);
  let problem = $state<string | null>(null);
  let attempt = $state(0);
  /** Shown from a saved copy: when it was saved, null for what loaded earlier in this visit. */
  let stale = $state<{ savedAt: number | null } | null>(null);

  const isList = (value: unknown): value is CommitteeSummary[] =>
    Array.isArray(value) && value.every((item) => isRecord(item) && typeof item.slug === "string");

  $effect(() => {
    void attempt;
    const controller = new AbortController();
    problem = null;
    loadPublicCommittees(controller.signal).then(
      (response) => {
        committees = response.committees;
        stale = null;
        remember(SAVED_KEY, response.committees);
      },
      (error: unknown) => {
        if (isAbortError(error)) return;
        // Keep what this visit already shows.
        if (committees !== null) {
          stale ??= { savedAt: null };
          return;
        }
        const saved = recall(SAVED_KEY, isList);
        if (saved !== null) {
          committees = saved.value;
          stale = { savedAt: saved.savedAt };
          return;
        }
        problem =
          isApiError(error) && error.code !== "network"
            ? error.message
            : "Check your connection and try again.";
      },
    );
    return () => controller.abort();
  });

  // Committees that share a room are listed together, in the site's order.
  const rooms = $derived.by(() => {
    const groups: { slugs: CommitteeSlug[]; room: string; mapUrl: string | null }[] = [];
    for (const committee of committees ?? []) {
      if (committee.meetingRoom === "") continue;
      const same = groups.find((group) => group.room === committee.meetingRoom);
      if (same === undefined) {
        groups.push({ slugs: [committee.slug], room: committee.meetingRoom, mapUrl: committee.mapUrl });
      } else {
        same.slugs.push(committee.slug);
      }
    }
    return groups;
  });

  const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Scrolls to a committee and moves keyboard focus there, as following an anchor should.
  function jump(event: MouseEvent, slug: string) {
    const target = document.querySelector<HTMLElement>(`#${CSS.escape(slug)}`);
    if (target === null) return;
    event.preventDefault();
    target.scrollIntoView({ behavior: reducedMotion() ? "auto" : "smooth", block: "start" });
    target.focus({ preventScroll: true });
  }

  function meets(committee: CommitteeSummary): string {
    if (committee.meetingTime === "") return committee.meetingRoom;
    if (committee.meetingRoom === "") return committee.meetingTime;
    return `${committee.meetingTime} in ${committee.meetingRoom}`;
  }
</script>

<svelte:head>
  <title>Committees - ScottyLabs</title>
  <meta
    name="description"
    content="Seven committees run ScottyLabs. See what each one builds, hosts or funds, and how to join."
  />
  <!-- Asked for from the HTML, so the committees and the first picture do not wait for the app. -->
  <link rel="preload" as="fetch" crossorigin="anonymous" href="/api/public/committees" />
</svelte:head>

<div class="index cb-scope">
  <section class="intro" aria-labelledby="committees-title">
    <p class="intro__label">[data-committees="7"]</p>
    <h1 id="committees-title" class="intro__title">Seven committees run ScottyLabs.</h1>
    <p class="intro__lead">
      Each one owns a part of how we build, host and fund things for campus. Pick one to see its teams, its
      work and how to join.
    </p>
    <nav class="intro__jump" aria-label="Jump to a committee">
      <ul>
        {#each COMMITTEE_LIST as committee (committee.slug)}
          <li>
            <a
              class="chip"
              href={`#${committee.slug}`}
              style:--dot={committee.color}
              onclick={(event) => jump(event, committee.slug)}
            >
              {committee.name}
            </a>
          </li>
        {/each}
      </ul>
    </nav>
  </section>

  {#if problem !== null}
    <section class="problem" role="alert">
      <h2 class="problem__title">We could not load the committees.</h2>
      <p class="problem__text">{problem}</p>
      <button type="button" class="cb-btn cb-btn--filled" onclick={() => (attempt += 1)}>Try again</button>
    </section>
  {:else if committees === null}
    <div class="rows" aria-busy="true">
      <p class="cb-sr-only" role="status">Loading the committees.</p>
      {#each COMMITTEE_LIST as committee, index (committee.slug)}
        <div id={committee.slug} class={["row", index % 2 === 1 && "row--flip"]} aria-hidden="true">
          <div class="row__cover"><Skeleton width="100%" height="100%" radius="14px" /></div>
          <div class="row__content">
            <p class="cb-inspector" style:--cb-accent={committee.color}>[committee="{committee.slug}"]</p>
            <p class="row__name">{committee.name}</p>
            <Skeleton width="100%" height="18px" />
            <Skeleton width="90%" height="18px" />
            <Skeleton width="60%" height="18px" />
          </div>
        </div>
      {/each}
    </div>
  {:else}
    {#if stale !== null}
      <LastGoodNote savedAt={stale.savedAt} onretry={() => (attempt += 1)} />
    {/if}
    <div class="rows">
      {#each committees as committee, index (committee.slug)}
        <article
          id={committee.slug}
          class={["row", "sl-select-host", index % 2 === 1 && "row--flip"]}
          tabindex="-1"
          aria-labelledby={`${committee.slug}-name`}
          style:--cb-accent={committee.color}
        >
          <div class="row__cover">
            {#if committee.cover}
              <Img
                image={committee.cover}
                alt=""
                class="cb-photo row__img"
                sizes="(min-width: 1280px) 696px, (min-width: 900px) 55vw, 100vw"
                fit="cover"
                loading={index === 0 ? "eager" : "lazy"}
                fetchpriority={index === 0 ? "high" : undefined}
              />
            {:else}
              <div class="row__no-cover" style:background={committee.tint}></div>
            {/if}
          </div>
          <div class="row__content">
            <p class="cb-inspector">[committee="{committee.slug}"]</p>
            <div class="row__title">
              <h2 id={`${committee.slug}-name`} class="row__name">{committee.name}</h2>
              {#if committee.indexDescription !== ""}
                <p class="row__description">{committee.indexDescription}</p>
              {/if}
            </div>
            <ul class="row__facts">
              {#if committee.teamsLine !== ""}
                <li><Users size={16} strokeWidth={2} aria-hidden="true" />{committee.teamsLine}</li>
              {/if}
              {#if meets(committee) !== ""}
                <li><MapPin size={16} strokeWidth={2} aria-hidden="true" />{meets(committee)}</li>
              {/if}
            </ul>
            <a class="cb-btn cb-btn--filled row__visit" href={committeePath(committee.slug)} data-selection-target>
              Visit {committee.name}
            </a>
          </div>
          <SelectionBox label={`/committees/${committee.slug}`} />
        </article>
      {/each}
    </div>

    {#if rooms.length > 0}
      <section class="band" aria-labelledby="sessions-title">
        <div class="band__inner">
          <div class="band__copy">
            <h2 id="sessions-title" class="band__title">Not sure yet? Come to a Saturday work session.</h2>
            <p class="band__text">
              Every committee works in Tepper on Saturdays from 4 to 6 PM. Drop in, meet the people and see what
              they are building before you pick.
            </p>
            <a class="cb-btn cb-btn--filled cb-btn--lg" href={JOIN_URL} target="_blank" rel="noopener noreferrer">
              Join ScottyLabs<span class="cb-sr-only"> (opens in a new tab)</span>
            </a>
          </div>
          <dl class="band__rooms">
            {#each rooms as group (group.room)}
              <div class="band__room">
                <dt>{committeeNames(group.slugs)}</dt>
                <dd>
                  {#if group.mapUrl}
                    <a href={group.mapUrl} target="_blank" rel="noopener noreferrer">
                      <MapPin size={16} strokeWidth={2} aria-hidden="true" />
                      {group.room}<span class="cb-sr-only"> in CMU Maps (opens in a new tab)</span>
                    </a>
                  {:else}
                    {group.room}
                  {/if}
                </dd>
              </div>
            {/each}
          </dl>
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  .index {
    --pad: 40px;
    padding-bottom: 0;
  }
  .intro,
  .rows,
  .problem {
    box-sizing: border-box;
    max-width: calc(1200px + 2 * var(--pad));
    margin: 0 auto;
    padding-inline: var(--pad);
  }
  .intro {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-top: 72px;
    padding-bottom: 32px;
  }
  .intro__label {
    margin: 0 0 8px;
    color: var(--cb-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .intro__title {
    max-width: 900px;
    margin: 0;
    color: var(--cb-text);
    font-size: 3.5rem;
    font-weight: 600;
    line-height: 62px;
    letter-spacing: -0.025em;
  }
  .intro__lead {
    max-width: 760px;
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 1.375rem;
    line-height: 32px;
  }
  .intro__jump ul {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 16px 0 0;
    list-style: none;
  }
  .intro__jump li {
    display: block;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px 8px 12px;
    border: 1px solid var(--cb-border);
    border-radius: 999px;
    background: #ffffff;
    color: var(--cb-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    text-decoration: none;
    transition: border-color var(--p-duration) var(--p-ease);

    &::before {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--dot);
      content: "";
    }
    &:hover {
      border-color: #9e9e9e;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 48px;
    padding-top: 40px;
    padding-bottom: 56px;
  }
  .row {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 696fr) minmax(0, 448fr);
    align-items: center;
    gap: 56px;
    margin-inline: -16px;
    padding: 16px;
    scroll-margin-top: 24px;

    &:focus {
      outline: none;
    }
  }
  .row--flip {
    grid-template-columns: minmax(0, 448fr) minmax(0, 696fr);
  }
  .row--flip .row__cover {
    order: 2;
  }
  /* Hiding overflow lets the aspect ratio win over the photo's own height. */
  .row__cover {
    overflow: hidden;
    aspect-ratio: 696 / 435;
  }
  .row__cover :global(.row__img) {
    border-radius: 14px;
  }
  .row__no-cover {
    width: 100%;
    height: 100%;
    border-radius: 14px;
  }
  .row__content {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .row__title {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row__name {
    margin: 0;
    color: var(--cb-text);
    font-size: 2.75rem;
    font-weight: 600;
    line-height: 50px;
    letter-spacing: -0.02em;
  }
  .row__description {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 1.125rem;
    line-height: 28px;
  }
  .row__facts {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: flex;
      align-items: flex-start;
      gap: 10px;
      color: var(--cb-text-secondary);
      font-size: 0.875rem;
      line-height: 20px;
    }
    & li > :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
  .row__visit {
    align-self: flex-start;
  }
  /* The whole row opens the committee page; the button is the one stop for the keyboard. */
  .row__visit::after {
    position: absolute;
    inset: 0;
    z-index: 1;
    content: "";
  }

  .problem {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    padding-top: 40px;
    padding-bottom: 96px;
  }
  .problem__title {
    margin: 0;
    color: var(--cb-text);
    font-size: 1.5rem;
    font-weight: 600;
    line-height: 32px;
  }
  .problem__text {
    margin: 0 0 8px;
    color: var(--cb-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }

  .band {
    background: #f5f5f5;
  }
  .band__inner {
    box-sizing: border-box;
    display: grid;
    grid-template-columns: minmax(0, 480fr) minmax(0, 624fr);
    gap: 96px;
    max-width: calc(1200px + 2 * var(--pad));
    margin: 0 auto;
    padding: 88px var(--pad);
  }
  .band__copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 20px;
  }
  .band__title {
    margin: 0;
    color: var(--cb-text);
    font-size: 2.75rem;
    font-weight: 600;
    line-height: 50px;
    letter-spacing: -0.02em;
  }
  .band__text {
    margin: 0 0 8px;
    color: var(--cb-text-secondary);
    font-size: 1.125rem;
    line-height: 28px;
  }
  .band__rooms {
    margin: 0;
  }
  .band__room {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 4px 24px;
    padding: 18px 0;
    border-top: 1px solid var(--cb-border);

    & dt {
      color: var(--cb-text);
      font-size: 1rem;
      font-weight: 500;
      line-height: 24px;
    }
    & dd {
      margin: 0;
      color: var(--cb-text-secondary);
      font-size: 1rem;
      line-height: 24px;
    }
    & a {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      color: var(--cb-text-secondary);
      text-decoration: underline;
      text-underline-offset: 3px;
    }
    & a:hover {
      color: var(--cb-text);
    }
    & a:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }

  @media (max-width: 899px) {
    .row,
    .row--flip {
      grid-template-columns: minmax(0, 1fr);
      gap: 20px;
    }
    .row--flip .row__cover {
      order: 0;
    }
    .row__content {
      gap: 16px;
    }
    .band__inner {
      grid-template-columns: minmax(0, 1fr);
      gap: 40px;
      padding-block: 56px;
    }
  }
  @media (max-width: 768px) {
    .index {
      --pad: 20px;
    }
    /* Figma 82:7402: 40 px from the chips to the first picture (24 + the row's 16) and 48 px
       between rows (16 + 2 x 16); the rows keep their padding for the hover box. */
    .intro {
      padding-top: 40px;
      padding-bottom: 24px;
    }
    .rows {
      gap: 16px;
      padding-top: 0;
    }
    .intro__title {
      font-size: 2.25rem;
      line-height: 42px;
    }
    .intro__lead {
      font-size: 1.125rem;
      line-height: 28px;
    }
    .row__cover {
      aspect-ratio: 350 / 220;
    }
    .row__name {
      font-size: 2.25rem;
      line-height: 42px;
    }
    .row__description {
      font-size: 1rem;
      line-height: 26px;
    }
    .band__title {
      font-size: 2rem;
      line-height: 38px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .chip {
      transition: none;
    }
  }
</style>
