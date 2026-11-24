<script lang="ts">
  import "#lib/portal/ui/table.css";
  import { onMount, tick, type Snippet } from "svelte";
  import { afterNavigate, goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onUnauthenticated } from "#lib/api/client.ts";
  import { portalRedirect } from "#lib/portal/access.ts";
  import { replaceLocation } from "#lib/portal/navigation.ts";
  import { isBarePortalPath } from "#lib/portal/paths.ts";
  import { PortalSession, setPortalSession } from "#lib/portal/session.svelte.ts";
  import { ToastStore, setToasts } from "#lib/portal/toasts.svelte.ts";
  import CheckingScreen from "#lib/portal/shell/CheckingScreen.svelte";
  import MobileBars from "#lib/portal/shell/MobileBars.svelte";
  import SessionError from "#lib/portal/shell/SessionError.svelte";
  import TopBar from "#lib/portal/shell/TopBar.svelte";
  import { LOCKED_PREVIEW_SECTIONS, sectionsFor } from "#lib/portal/shell/sections.ts";
  import ToastRegion from "#lib/portal/ui/ToastRegion.svelte";

  let { children }: { children: Snippet } = $props();

  const session = setPortalSession(new PortalSession());
  const toasts = setToasts(new ToastStore());

  const bare = $derived(isBarePortalPath(page.url.pathname));
  const hasRole = $derived(session.canUsePortal);
  const sections = $derived(hasRole ? sectionsFor(session.permissions) : LOCKED_PREVIEW_SECTIONS);
  const redirectTarget = $derived(portalRedirect(page.url, session));
  // Render the page only when the person may see it, so pages never flash before a redirect.
  const showShell = $derived(session.status === "ready" && redirectTarget === null);

  $effect(() => {
    if (!bare && session.status === "idle") void session.load();
  });

  // Normally the sign-in page already chose the right place. This catches everything else, and
  // falls back to a full page load when the router does not get there.
  $effect(() => {
    const target = redirectTarget;
    if (target === null) return;
    void replaceLocation(
      target,
      () => redirectTarget === target,
      (href) => goto(href, { replace: true }),
    );
  });

  // A 401 from any call means the session ended while the person was working.
  onMount(() =>
    onUnauthenticated(() => {
      if (session.status === "ready") session.markSignedOut("expired");
    }),
  );

  onMount(() => () => toasts.clear());

  // Move focus to the new page's heading so keyboard and screen reader users start there.
  afterNavigate(async (navigation) => {
    if (navigation.type === "enter") return;
    if (navigation.from?.url.pathname === navigation.to?.url.pathname) return;
    await tick();
    const main = document.querySelector<HTMLElement>("#portal-main");
    const heading = main?.querySelector("h1");
    const target = heading ?? main;
    if (target === null || target === undefined) return;
    if (!target.hasAttribute("tabindex")) target.setAttribute("tabindex", "-1");
    target.focus({ preventScroll: true });
  });
</script>

<svelte:head>
  <title>ScottyLabs Portal</title>
  <meta name="robots" content="noindex" />
</svelte:head>

{#if bare}
  {@render children()}
{:else if session.status === "error"}
  <SessionError error={session.error} onretry={() => void session.load()} />
{:else if showShell}
  <div class="portal">
    <a class="portal__skip" href="#portal-main">Skip to content</a>
    <div class="portal__desktop">
      <TopBar {sections} locked={!hasRole} />
    </div>
    <div class="portal__mobile">
      <MobileBars {sections} locked={!hasRole} />
    </div>
    <main id="portal-main" class={["portal__main", !hasRole && "portal__main--locked"]} tabindex="-1">
      {@render children()}
    </main>
    <ToastRegion store={toasts} />
  </div>
{:else}
  <CheckingScreen />
{/if}

<style>
  .portal {
    min-height: 100vh;
    min-height: 100dvh;
    background: var(--p-bg);
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
    --p-toast-offset: 24px;
  }
  .portal__desktop,
  .portal__mobile {
    display: contents;
  }
  .portal__main {
    box-sizing: border-box;
    width: 100%;
    max-width: calc(var(--p-content-width) + 80px);
    margin: 0 auto;
    padding: 36px 40px 72px;

    &:focus {
      outline: none;
    }
  }
  .portal__skip {
    position: fixed;
    top: 8px;
    left: 8px;
    z-index: 100;
    padding: 10px 16px;
    border-radius: var(--p-radius);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-size: 0.875rem;
    font-weight: 500;
    transform: translateY(-200%);

    &:focus-visible {
      transform: none;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  @media (min-width: 900px) {
    .portal__mobile {
      display: none;
    }
  }
  @media (max-width: 899px) {
    .portal {
      --p-toast-offset: calc(var(--p-tabbar-height) + 12px);
    }
    .portal__desktop {
      display: none;
    }
    .portal__main {
      padding: 20px 16px calc(var(--p-tabbar-height) + 32px + env(safe-area-inset-bottom));
    }
    .portal__main--locked {
      padding-bottom: 40px;
    }
  }
</style>
