<script lang="ts">
  // Phones: a short top bar with the brand and account, and the sections in a bottom tab bar.
  import { page } from "$app/state";
  import { PORTAL_HOME } from "../paths.ts";
  import DogMark from "../ui/DogMark.svelte";
  import AccountMenu from "./AccountMenu.svelte";
  import { isSectionActive, type PortalSection } from "./sections.ts";

  let {
    sections,
    locked = false,
  }: { sections: readonly PortalSection[]; locked?: boolean } = $props();
</script>

<header class="mobile-top">
  <a class="mobile-top__brand" href={PORTAL_HOME}>
    <DogMark width={26} />
    <span>Portal</span>
    <span class="p-sr-only">, ScottyLabs</span>
  </a>
  <AccountMenu compact />
</header>

{#if !locked}
  <nav class="tabbar" aria-label="Portal sections">
    <ul>
      {#each sections as section (section.id)}
        {@const active = isSectionActive(section, page.url.pathname)}
        <li>
          <a
            class={["tabbar__item", active && "tabbar__item--active"]}
            href={section.href}
            aria-current={active ? "page" : undefined}
          >
            <section.icon size={20} strokeWidth={active ? 2 : 1.75} aria-hidden="true" />
            <span class="tabbar__label">{section.shortLabel}</span>
            {#if section.shortLabel !== section.label}
              <span class="p-sr-only">({section.label})</span>
            {/if}
          </a>
        </li>
      {/each}
    </ul>
  </nav>
{/if}

<style>
  .mobile-top {
    position: sticky;
    top: 0;
    z-index: 40;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--p-topbar-height-mobile);
    padding: 0 12px 0 16px;
    border-bottom: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .mobile-top__brand {
    display: inline-flex;
    align-items: center;
    gap: 8px;
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
  .tabbar {
    position: fixed;
    inset: auto 0 0;
    z-index: 40;
    box-sizing: border-box;
    padding: 6px 8px calc(6px + env(safe-area-inset-bottom));
    border-top: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .tabbar ul {
    display: flex;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .tabbar li {
    display: flex;
    flex: 1;
  }
  .tabbar__item {
    box-sizing: border-box;
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    min-height: 50px;
    border-radius: var(--p-radius);
    color: var(--p-text-tertiary);
    font-size: 0.6875rem;
    font-weight: 500;
    line-height: 13px;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -2px;
    }
  }
  .tabbar__item--active {
    color: var(--p-text);
    font-weight: 700;
  }
</style>
