<script lang="ts">
  // Committee pages in the portal (spec CP-1): the seven pages with their status, last edit and
  // director. People see "Edit page" only where they may edit; elsewhere a lock and "View".
  import Lock from "@lucide/svelte/icons/lock";
  import { page as current } from "$app/state";
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { CommitteePageSummary } from "#lib/api/types.ts";
  import { loadPages } from "#lib/committees/api.ts";
  import { STATUS_LABELS, STATUS_TONES } from "#lib/committees/status.ts";
  import Img from "#lib/media/Img.svelte";
  import { committeeNames, formatUpdated, getPortalSession } from "#lib/portal/index.ts";
  import { Avatar, Button, Callout, PageHeader, Skeleton, StatusDot } from "#lib/portal/ui/index.ts";

  const session = getPortalSession();
  // The page just published from the editor (spec F7).
  const highlight = $derived(current.url.searchParams.get("highlight"));
  let pages = $state.raw<CommitteePageSummary[] | null>(null);
  let problem = $state<string | null>(null);
  let attempt = $state(0);
  const now = new Date();

  $effect(() => {
    void attempt;
    const controller = new AbortController();
    problem = null;
    loadPages(controller.signal).then(
      (response) => {
        pages = response.pages;
      },
      (error: unknown) => {
        if (isAbortError(error)) return;
        problem = isApiError(error) ? error.message : "We could not load the committee pages. Try again.";
      },
    );
    return () => controller.abort();
  });

  const permissions = $derived(session.permissions);
  const isAdmin = $derived(permissions?.isAdmin ?? false);
  const editable = $derived(permissions?.editablePages ?? []);
  const description = $derived.by(() => {
    if (isAdmin) return "Seven pages, one per committee. Directors edit their own.";
    if (editable.length === 0) return "Each committee\u2019s page, as it is now. Only its director or an admin can edit it.";
    const pagesWord = editable.length === 1 ? "page" : "pages";
    return `You can edit the ${committeeNames(editable)} ${pagesWord}. The others are here to look at.`;
  });
  const footnote = $derived(
    isAdmin
      ? "Each page is built from the same blocks, so every committee matches the site."
      : "Only the director of a committee, or an admin, can edit its page.",
  );

  function editorPath(page: CommitteePageSummary): string {
    return `/portal/committees/${page.slug}/edit/`;
  }
</script>

<svelte:head>
  <title>Committee pages - ScottyLabs Portal</title>
</svelte:head>

<PageHeader title="Committee pages" {description} />

{#if problem !== null}
  <div class="pages__body">
    <Callout tone="error" title="We could not load the committee pages." role="alert">
      {problem}
      {#snippet actions()}
        <Button variant="outline" size="sm" onclick={() => (attempt += 1)}>Try again</Button>
      {/snippet}
    </Callout>
  </div>
{:else}
  <div class="p-table-card pages__body">
    <table class="p-table p-table--stack pages">
      <thead>
        <tr>
          <th scope="col">Page</th>
          <th scope="col">Status</th>
          <th scope="col">Last edited</th>
          <th scope="col">Director</th>
          <th scope="col"><span class="p-sr-only">Actions</span></th>
        </tr>
      </thead>
      <tbody>
        {#if pages === null}
          {#each { length: 7 } as _, index (index)}
            <tr aria-hidden="true">
              <td class="p-table__cell--full" colspan="5"><Skeleton height="56px" /></td>
            </tr>
          {/each}
        {:else}
          {#each pages as page (page.slug)}
            <tr class={[page.slug === highlight && "p-table__row--highlight"]}>
              <td data-label="Page">
                <span class="p-table__primary">
                  {#if page.cover}
                    <Img image={page.cover} alt="" class="p-table__thumb pages__thumb" sizes="88px" fallbackWidth={480} fit="cover" />
                  {:else}
                    <span class="p-table__thumb pages__thumb" style:background={page.tint} aria-hidden="true"></span>
                  {/if}
                  <span>
                    <span class="p-table__title">{page.name}</span>
                    <span class="p-table__subtitle">/committees/{page.slug}</span>
                  </span>
                </span>
              </td>
              <td data-label="Status">
                <StatusDot tone={STATUS_TONES[page.status]} label={STATUS_LABELS[page.status]} />
              </td>
              <td data-label="Last edited" class="p-table__muted">
                <time datetime={page.draftUpdatedAt}>{formatUpdated(page.draftUpdatedAt, now)}</time>
              </td>
              <td data-label="Director">
                {#if page.directors.length === 0}
                  <span class="p-table__muted">No director yet</span>
                {:else}
                  <span class="pages__directors">
                    {#each page.directors as director (director.id)}
                      <span class="pages__director">
                        <Avatar initials={director.initials} committee={page.slug} size="sm" />
                        {director.name}
                      </span>
                    {/each}
                  </span>
                {/if}
              </td>
              <td class="p-table__actions">
                {#if page.canEdit}
                  <Button
                    href={editorPath(page)}
                    variant={isAdmin ? "outline" : "filled"}
                    size="sm"
                    aria-label={`Edit the ${page.name} page`}
                  >
                    Edit page
                  </Button>
                {:else}
                  <!-- The footnote under the table says why for everyone; the tooltip repeats it here. -->
                  <span class="pages__locked" title={`Only the director of ${page.name} or an admin can edit this page.`}>
                    <Lock size={16} strokeWidth={2} aria-hidden="true" />
                    <span class="p-sr-only">Only the director of {page.name} or an admin can edit this page.</span>
                    <Button href={editorPath(page)} variant="subtle" size="sm" aria-label={`View the ${page.name} page`}>
                      View
                    </Button>
                  </span>
                {/if}
              </td>
            </tr>
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
  <p class="p-table-footnote">{footnote}</p>
{/if}

<style>
  /* The gap between the page header and the table, as on the other portal lists (Figma 107:4464). */
  .pages__body {
    margin-top: 24px;
  }
  .pages :global(.pages__thumb) {
    width: 88px;
    height: 56px;
  }
  .pages__directors {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pages__director {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
  }
  .pages__locked {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    color: var(--p-text-secondary);
  }
</style>
