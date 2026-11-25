<script lang="ts">
  // The saved draft as visitors would see its page once published (spec PR-4), at full width.
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import { type ApiError, get, isApiError } from "#lib/api/client.ts";
  import type { ProjectEditorResponse } from "#lib/api/types.ts";
  import PortalError from "#lib/portal/PortalError.svelte";
  import { Button, Skeleton } from "#lib/portal/ui/index.ts";
  import ProjectPage from "#lib/projects/ProjectPage.svelte";
  import { draftDetail } from "#lib/projects/drafts.ts";

  const id = $derived(page.params.id ?? "");
  let project = $state.raw<ProjectEditorResponse["project"] | null>(null);
  let loadError = $state.raw<ApiError | null>(null);

  const detail = $derived(
    project === null
      ? null
      : draftDetail(project.draft, { id: project.id, slug: project.slug, featured: project.featured }),
  );

  $effect(() => {
    const current = id;
    untrack(() => void load(current));
  });

  async function load(current: string): Promise<void> {
    project = null;
    loadError = null;
    try {
      project = (await get<ProjectEditorResponse>(`/projects/${encodeURIComponent(current)}`)).project;
    } catch (error) {
      if (isApiError(error)) loadError = error;
    }
  }
</script>

<svelte:head>
  <title>Preview {project?.draft.name ?? "project"} - ScottyLabs Portal</title>
</svelte:head>

{#if loadError !== null}
  <PortalError status={loadError.status} message={loadError.message} />
{:else}
  <div class="bar">
    <div class="bar__text">
      <h1 class="bar__title" tabindex="-1">Preview</h1>
      <p class="bar__note">
        This is the saved draft. Visitors see it only after it is published.
      </p>
    </div>
    <Button variant="outline" icon={ArrowLeft} href={`/portal/projects/${id}/`}>Back to editing</Button>
  </div>

  <div class="canvas">
    {#if detail !== null}
      <ProjectPage project={detail} preview />
    {:else}
      <p class="p-sr-only" role="status">Loading the preview</p>
      <Skeleton width="100%" height="480px" radius="20px" />
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
    padding: 16px 20px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .bar__text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .bar__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    line-height: 28px;

    &:focus {
      outline: none;
    }
  }
  .bar__note {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .canvas {
    margin-top: 20px;
    padding: 28px clamp(16px, 4vw, 48px) 64px;
    border-radius: 20px;
    background: #fff;
  }
</style>
