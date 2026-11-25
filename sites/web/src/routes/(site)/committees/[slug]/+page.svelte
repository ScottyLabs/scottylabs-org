<script lang="ts">
  // One committee's page (spec C-6 to C-11): the published blocks, loaded in the browser. A
  // skeleton holds the space while it loads; a page that is not published yet says so.
  import "#lib/committees/committees.css";
  import Lock from "@lucide/svelte/icons/lock";
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { PublicCommitteePageResponse } from "#lib/api/types.ts";
  import { loadPublicPage } from "#lib/committees/api.ts";
  import CommitteePage from "#lib/committees/CommitteePage.svelte";
  import PageSkeleton from "#lib/committees/PageSkeleton.svelte";
  import { COMMITTEES } from "#lib/portal/committees.ts";
  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  const style = $derived(COMMITTEES[data.slug]);
  let page = $state.raw<PublicCommitteePageResponse | null>(null);
  let status = $state<"loading" | "ready" | "missing" | "error">("loading");
  let problem = $state("");
  let attempt = $state(0);

  $effect(() => {
    const slug = data.slug;
    void attempt;
    const controller = new AbortController();
    status = "loading";
    loadPublicPage(slug, controller.signal).then(
      (response) => {
        page = response;
        status = "ready";
      },
      (error: unknown) => {
        if (isAbortError(error)) return;
        if (isApiError(error) && error.status === 404) {
          status = "missing";
          return;
        }
        problem = isApiError(error) && error.code !== "network" ? error.message : "Check your connection and try again.";
        status = "error";
      },
    );
    return () => controller.abort();
  });

  const description = $derived(page?.committee.indexDescription ?? `The ${style.name} committee at ScottyLabs.`);
</script>

<svelte:head>
  <title>{style.name} - ScottyLabs</title>
  <meta name="description" content={description} />
</svelte:head>

<div class="page cb-scope" style:--cb-accent={style.color}>
  {#if status === "ready" && page}
    <CommitteePage
      committee={{ slug: page.committee.slug, name: page.committee.name, color: page.committee.color, tint: page.committee.tint }}
      settings={page.committee}
      blocks={page.blocks}
      projects={page.projects}
    />
  {:else if status === "missing"}
    <section class="notice" aria-labelledby="notice-title">
      <p class="cb-inspector">[committee="{data.slug}"]</p>
      <h1 id="notice-title" class="notice__title">{style.name}</h1>
      <p class="notice__text">This page is not published yet. Come back soon, or meet the other committees.</p>
      <a class="cb-btn cb-btn--filled" href="/committees/">See all committees</a>
    </section>
  {:else if status === "error"}
    <section class="notice" aria-labelledby="notice-title" role="alert">
      <h1 id="notice-title" class="notice__title notice__title--small">We could not load the {style.name} page.</h1>
      <p class="notice__text">{problem}</p>
      <button type="button" class="cb-btn cb-btn--filled" onclick={() => (attempt += 1)}>Try again</button>
    </section>
  {:else}
    <PageSkeleton name={style.name} slug={data.slug} />
  {/if}
</div>

<p class="sign-in">
  <Lock size={14} strokeWidth={2} aria-hidden="true" />
  <span>Director of {style.name}? <a href={`/portal/committees/${data.slug}/edit/`}>Sign in to edit this page.</a></span>
</p>

<style>
  .page {
    box-sizing: border-box;
    width: 100%;
    max-width: 1280px;
    margin: 0 auto;
    padding: 36px 40px 96px;
  }
  .notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
    min-height: 50vh;
    padding-top: 24px;
  }
  .notice__title {
    margin: 0;
    color: var(--cb-text);
    font-size: 3.5rem;
    font-weight: 600;
    line-height: 62px;
    letter-spacing: -0.025em;
  }
  .notice__title--small {
    font-size: 2rem;
    line-height: 40px;
  }
  .notice__text {
    max-width: 560px;
    margin: 0 0 8px;
    color: var(--cb-text-secondary);
    font-size: 1.125rem;
    line-height: 28px;
  }
  .sign-in {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin: 0;
    padding: 18px 20px;
    border-top: 1px solid #d6d6d6;
    color: var(--p-text-tertiary);
    font-family: var(--p-font);
    font-size: 0.75rem;
    line-height: 16px;
    text-align: center;

    & a {
      color: inherit;
      text-decoration: none;
    }
    & a:hover {
      color: #1e1e1e;
      text-decoration: underline;
    }
    & a:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }

  @media (max-width: 768px) {
    .page {
      padding: 20px 20px 64px;
    }
    .notice__title {
      font-size: 2.5rem;
      line-height: 46px;
    }
  }
</style>
