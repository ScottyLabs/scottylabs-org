<script lang="ts">
  // "Announcements > New": the way back to the list, plus the page's h1 for the shell to focus.
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  let { title, crumb }: { title: string; crumb: string } = $props();
</script>

<h1 class="p-sr-only" tabindex="-1">{title}</h1>
<nav class="crumbs" aria-label="Breadcrumb">
  <ol>
    <li><a href="/portal/ticker/">Announcements</a></li>
    <li aria-current="page">
      <ChevronRight size={14} strokeWidth={2} aria-hidden="true" />
      <span>{crumb}</span>
    </li>
  </ol>
</nav>

<style>
  .crumbs ol {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.875rem;
    line-height: 20px;
  }
  .crumbs li {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--p-text);
  }
  .crumbs a {
    border-radius: 4px;
    color: var(--p-text-secondary);
    text-decoration: none;

    &:hover {
      color: var(--p-text);
      text-decoration: underline;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
</style>
