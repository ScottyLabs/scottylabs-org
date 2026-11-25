<script lang="ts">
  // The page editor for one committee. Loads the draft, then hands it to the editor; anyone in the
  // portal can open it, and people who cannot edit get it read only.
  import { isAbortError, isApiError } from "#lib/api/client.ts";
  import type { CommitteePageEditorResponse } from "#lib/api/types.ts";
  import { loadEditor } from "#lib/committees/api.ts";
  import Editor from "#lib/committees/editor/Editor.svelte";
  import { committeeName } from "#lib/portal/committees.ts";
  import { Button, Callout, PageHeader, Skeleton } from "#lib/portal/ui/index.ts";
  import type { PageProps } from "./$types";

  let { data }: PageProps = $props();

  let response = $state.raw<CommitteePageEditorResponse | null>(null);
  let problem = $state<string | null>(null);
  let attempt = $state(0);
  const name = $derived(committeeName(data.slug));

  $effect(() => {
    const slug = data.slug;
    void attempt;
    const controller = new AbortController();
    problem = null;
    response = null;
    loadEditor(slug, controller.signal).then(
      (loaded) => {
        response = loaded;
      },
      (error: unknown) => {
        if (isAbortError(error)) return;
        problem = isApiError(error) ? error.message : "We could not load the page. Try again.";
      },
    );
    return () => controller.abort();
  });
</script>

<svelte:head>
  <title>Edit the {name} page - ScottyLabs Portal</title>
</svelte:head>

{#if response !== null}
  {#key response.committee.slug}
    <Editor {response} />
  {/key}
{:else if problem !== null}
  <PageHeader title={`${name} committee page`} />
  <Callout tone="error" title="We could not open the editor." role="alert">
    {problem}
    {#snippet actions()}
      <Button variant="outline" size="sm" onclick={() => (attempt += 1)}>Try again</Button>
      <Button variant="subtle" size="sm" href="/portal/committees/">Back to committee pages</Button>
    {/snippet}
  </Callout>
{:else}
  <PageHeader title={`${name} committee page`} description="Opening the editor." />
  <div aria-busy="true"><Skeleton height="420px" radius="16px" /></div>
{/if}
