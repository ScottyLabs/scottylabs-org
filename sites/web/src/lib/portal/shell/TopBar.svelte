<script lang="ts">
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import { page } from "$app/state";
  import { PORTAL_HOME } from "../paths.ts";
  import Button from "../ui/Button.svelte";
  import DogMark from "../ui/DogMark.svelte";
  import AccountMenu from "./AccountMenu.svelte";
  import { isSectionActive, type PortalSection } from "./sections.ts";

  let {
    sections,
    locked = false,
  }: {
    sections: readonly PortalSection[];
    /** For people without a role: sections are shown greyed out and cannot be opened. */
    locked?: boolean;
  } = $props();
</script>

<header class="topbar">
  <div class="topbar__inner">
    <div class="topbar__left">
      <a class="topbar__brand" href={PORTAL_HOME}>
        <DogMark width={27} />
        <span>Portal</span>
        <span class="p-sr-only">, ScottyLabs</span>
      </a>
      {#if locked}
        <div class="topbar__nav topbar__nav--locked" aria-hidden="true">
          {#each sections as section (section.id)}
            <span class="topbar__item">
              <section.icon size={16} strokeWidth={1.75} />
              {section.label}
            </span>
          {/each}
        </div>
      {:else}
        <nav class="topbar__nav" aria-label="Portal sections">
          <ul>
            {#each sections as section (section.id)}
              {@const active = isSectionActive(section, page.url.pathname)}
              <li>
                <a
                  class={["topbar__item", active && "topbar__item--active"]}
                  href={section.href}
                  aria-current={active ? "page" : undefined}
                >
                  <section.icon size={16} strokeWidth={1.75} aria-hidden="true" />
                  {section.label}
                </a>
              </li>
            {/each}
          </ul>
        </nav>
      {/if}
    </div>
    <div class="topbar__right">
      <Button variant="outline" size="sm" icon={ExternalLink} href="/" target="_blank">
        View site<span class="p-sr-only"> (opens in a new tab)</span>
      </Button>
      <AccountMenu />
    </div>
  </div>
</header>

<style>
  .topbar {
    position: sticky;
    top: 0;
    z-index: 40;
    border-bottom: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .topbar__inner {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    /* The bottom border makes up the rest of the bar's height. */
    height: calc(var(--p-topbar-height) - 1px);
    padding: 0 40px;
  }
  .topbar__left {
    display: flex;
    align-items: center;
    gap: 32px;
    min-width: 0;
  }
  .topbar__brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    flex: none;
    color: var(--p-text);
    font-size: 1.0625rem;
    font-weight: 500;
    line-height: 24px;
    border-radius: 6px;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 4px;
    }
  }
  .topbar__nav,
  .topbar__nav ul {
    display: flex;
    align-items: center;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .topbar__nav li {
    display: flex;
  }
  .topbar__item {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: var(--p-radius);
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
    white-space: nowrap;
    transition:
      background-color var(--p-duration) var(--p-ease),
      color var(--p-duration) var(--p-ease);

    & :global(svg) {
      color: var(--p-text);
    }
  }
  a.topbar__item {
    &:hover {
      background: var(--p-surface-muted);
      color: var(--p-text);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }
  .topbar__item--active,
  a.topbar__item--active:hover {
    border-color: var(--p-border);
    background: var(--p-surface-strong);
    color: var(--p-text);
  }
  .topbar__nav--locked .topbar__item {
    color: #b3b3b3;
    & :global(svg) {
      color: #b3b3b3;
    }
  }
  .topbar__right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
  }
</style>
