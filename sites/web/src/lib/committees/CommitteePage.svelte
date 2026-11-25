<script lang="ts">
  // A committee page as visitors see it: the breadcrumb and the blocks. The public route and the
  // editor's preview both use it, so the preview is exactly the page.
  import "./committees.css";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import type { Block, CommitteeSettings, ProjectCard } from "../api/types.ts";
  import BlockRenderer from "./BlockRenderer.svelte";
  import type { CommitteeInfo } from "./render/types.ts";

  let {
    committee,
    settings,
    blocks,
    projects,
  }: {
    committee: CommitteeInfo;
    settings: CommitteeSettings;
    blocks: readonly Block[];
    projects: readonly ProjectCard[];
  } = $props();
</script>

<div class="cb-scope committee-page">
  <nav class="committee-page__crumbs" aria-label="Breadcrumb">
    <ol>
      <li><a href="/committees/">Committees</a></li>
      <li aria-hidden="true"><ChevronRight size={14} strokeWidth={2} /></li>
      <li><span aria-current="page">{committee.name}</span></li>
    </ol>
  </nav>
  <BlockRenderer {blocks} {committee} {settings} {projects} />
</div>

<style>
  .committee-page {
    display: flex;
    flex-direction: column;
    gap: 40px;
  }
  .committee-page__crumbs ol {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .committee-page__crumbs li {
    display: flex;
    align-items: center;
    color: var(--cb-text);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .committee-page__crumbs li:first-child a {
    color: var(--cb-text-secondary);
    text-decoration: none;

    &:hover {
      color: var(--cb-text);
      text-decoration: underline;
    }
    &:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .committee-page__crumbs li[aria-hidden="true"] {
    color: var(--cb-text-tertiary);
  }

  @media (max-width: 640px) {
    .committee-page {
      gap: 24px;
    }
  }
</style>
