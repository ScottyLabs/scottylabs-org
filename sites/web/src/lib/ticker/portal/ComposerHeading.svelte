<script lang="ts">
  // The top of the composer and the edit page, with the page's h1 for the shell to focus.
  // Wide screens show "Announcements > New" and keep the h1 for screen readers; phones show an
  // app bar with back, the title and Cancel, as in the mobile design.
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  const LIST = "/portal/ticker/";

  let { title, crumb }: { title: string; crumb: string } = $props();
</script>

<header class="bar">
  <a class="bar__back" href={LIST}>
    <ChevronLeft size={20} strokeWidth={2} aria-hidden="true" />
    <span class="p-sr-only">Back to announcements</span>
  </a>
  <h1 class="bar__title" tabindex="-1">{title}</h1>
  <a class="bar__cancel" href={LIST}>Cancel</a>
</header>
<nav class="crumbs" aria-label="Breadcrumb">
  <ol>
    <li><a href={LIST}>Announcements</a></li>
    <li aria-current="page">
      <ChevronRight size={14} strokeWidth={2} aria-hidden="true" />
      <span>{crumb}</span>
    </li>
  </ol>
</nav>

<style>
  .bar {
    display: contents;
  }
  .bar__back,
  .bar__cancel {
    display: none;
  }
  .bar__title {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
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

  /* Edge to edge under the shell's top bar, past its 20 px and 16 px padding. */
  @media (max-width: 600px) {
    .bar {
      box-sizing: border-box;
      display: flex;
      align-items: center;
      gap: 4px;
      height: 56px;
      margin: -20px -16px 0;
      padding: 0 12px 0 8px;
      border-bottom: 1px solid var(--p-border);
      background: var(--p-surface);
    }
    .bar__back,
    .bar__cancel {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-height: 40px;
      border-radius: var(--p-radius-sm);
      color: var(--p-text);
      text-decoration: none;

      &:focus-visible {
        outline: 2px solid var(--p-focus);
        outline-offset: -2px;
      }
    }
    .bar__back {
      flex: none;
      width: 32px;
    }
    .bar__title {
      position: static;
      flex: 1;
      width: auto;
      height: auto;
      min-width: 0;
      margin: 0;
      overflow: hidden;
      clip-path: none;
      color: var(--p-text);
      font-size: 1rem;
      font-weight: 500;
      line-height: 24px;
      text-overflow: ellipsis;

      &:focus {
        outline: none;
      }
    }
    .bar__cancel {
      flex: none;
      padding: 0 4px;
      color: var(--p-text-secondary);
      font-size: 0.875rem;
      line-height: 20px;

      &:hover {
        color: var(--p-text);
      }
    }
    .crumbs {
      display: none;
    }
  }
</style>
