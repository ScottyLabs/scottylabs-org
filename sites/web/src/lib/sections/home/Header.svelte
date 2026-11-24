<script lang="ts">
  import type { Component } from "svelte";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import Menu from "@lucide/svelte/icons/menu";
  import SquareKanban from "@lucide/svelte/icons/square-kanban";
  import X from "@lucide/svelte/icons/x";
  import { afterNavigate } from "$app/navigation";
  import { page } from "$app/state";
  import scottylabsLogo from "#lib/assets/scottylabs-logo.svg";
  import teamIcon from "#lib/assets/icons/team.svg";
  import sponsorsIcon from "#lib/assets/icons/sponsors.svg";

  type NavIcon = { image: string } | { component: Component<{ class?: string; strokeWidth?: number }> };

  const navLinks: { icon: NavIcon; url: string; text: string }[] = [
    { icon: { image: teamIcon }, url: "/team/", text: "Team" },
    { icon: { image: sponsorsIcon }, url: "/sponsors/", text: "Sponsors" },
    { icon: { component: SquareKanban }, url: "/projects/", text: "Projects" },
    { icon: { component: LayoutGrid }, url: "/committees/", text: "Committees" },
  ];

  // The phone menu is a popover, so the button opens it before the page hydrates and without
  // JavaScript at all. The browser closes it on Escape or a click outside and returns focus to
  // the button; only closing it after a client-side navigation needs a script.
  let menu: HTMLElement | undefined = $state();

  const isActive = (url: string) => page.url.pathname.startsWith(url);

  afterNavigate((navigation) => {
    if (navigation.type !== "enter" && menu?.matches(":popover-open")) menu.hidePopover();
  });
</script>

{#snippet navItems(variant: "bar" | "menu")}
  {#each navLinks as { icon, url, text } (url)}
    <a
      class={["nav-button", variant === "menu" && "nav-button--menu", isActive(url) && "nav-button--active"]}
      href={url}
      aria-current={isActive(url) ? "page" : undefined}
    >
      {#if "image" in icon}
        <img class="nav-button__icon" src={icon.image} alt="" />
      {:else}
        <icon.component class="nav-button__icon nav-button__icon--lucide" strokeWidth={2} />
      {/if}
      <div class="nav-button__text">{text}</div>
    </a>
  {/each}
{/snippet}

<header class="main-header-container">
  <div class="main-header-layout">
    <a class="logo" href="/">
      <img class="logo__img" src={scottylabsLogo} alt="" />

      <div class="logo__text">ScottyLabs</div>
    </a>
    <nav class="main-nav" aria-label="Main">
      {@render navItems("bar")}
    </nav>
    <!-- The browser reports whether the menu is open (aria-expanded) for popover buttons. -->
    <button class="menu-toggle" type="button" popovertarget="site-menu" aria-label="Menu">
      <Menu class="menu-toggle__icon menu-toggle__icon--open" size={22} strokeWidth={2} aria-hidden="true" />
      <X class="menu-toggle__icon menu-toggle__icon--close" size={22} strokeWidth={2} aria-hidden="true" />
    </button>
  </div>
  <nav id="site-menu" class="menu-panel" aria-label="Main" popover="auto" bind:this={menu}>
    {@render navItems("menu")}
  </nav>
</header>

<style>
  .main-header-container {
    anchor-name: --site-header;
    position: sticky;
    top: 0;
    backdrop-filter: blur(40px);
    background: rgba(255, 255, 255, 0.809);
    z-index: 999;
  }
  .main-header-layout {
    max-width: 1400px;
    margin-inline: auto;
    padding: 20px 40px;
    @media (max-width: 768px) {
      padding: 14px 20px;
    }
    display: flex;
  }

  .logo {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: var(--text-xl);
    font-weight: 500;
    margin-right: auto;
  }

  .logo__img {
    height: 1.5em;
  }
  .logo__text {
    flex-grow: 0;
    color: var(--black-900);
  }
  .main-nav {
    display: flex;
    gap: 2px;
  }
  .nav-button {
    display: flex;
    gap: 4px;
    padding: 10px 13px;

    font-size: var(--text-base);
    align-items: center;
    color: var(--black-900);
    border: 1px solid transparent;
    border-radius: var(--border-radius-base);

    &:hover {
      background-color: var(--black-50);
    }
    &:focus-visible {
      outline: 2px solid var(--blue-600);
      outline-offset: 2px;
    }
  }

  .nav-button--active {
    background-color: var(--blue-50);
    border-color: var(--black-200);
  }
  .nav-button__icon {
    height: 1.1em;
  }
  .nav-button :global(.nav-button__icon--lucide) {
    width: 1.1em;
    height: 1.1em;
    flex: none;
  }

  .menu-toggle {
    display: none;
    align-self: center;
    place-items: center;
    width: 40px;
    height: 40px;
    margin-right: -8px;
    border-radius: var(--border-radius-base);
    color: var(--black-900);
    &:hover {
      background-color: var(--black-50);
    }
    &:focus-visible {
      outline: 2px solid var(--blue-600);
      outline-offset: 2px;
    }
  }
  .menu-toggle :global(.menu-toggle__icon--close),
  .main-header-container:has(.menu-panel:popover-open) .menu-toggle :global(.menu-toggle__icon--open) {
    display: none;
  }
  .main-header-container:has(.menu-panel:popover-open) .menu-toggle :global(.menu-toggle__icon--close) {
    display: block;
  }
  .menu-panel {
    display: none;
  }

  @media (max-width: 699px) {
    .main-nav {
      display: none;
    }
    .menu-toggle {
      display: grid;
    }
    /* Popovers sit in the top layer, fixed to the viewport. The header is sticky at the top, so
       the panel goes right under it: at its anchored bottom edge, or its phone height where
       anchor positioning is not supported. */
    .menu-panel:popover-open {
      position: fixed;
      inset: 68px 0 auto;
      top: anchor(--site-header bottom);
      box-sizing: border-box;
      width: auto;
      max-height: calc(100dvh - 68px);
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: 4px;
      padding: 4px 20px 16px;
      border: 0;
      border-bottom: 1px solid var(--black-200);
      background: white;
      color: var(--black-900);
      box-shadow: 0 12px 24px rgba(0, 0, 0, 0.08);
    }
    .nav-button--menu {
      padding: 12px 13px;
      gap: 10px;
    }
  }
</style>
