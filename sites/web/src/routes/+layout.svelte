<script lang="ts">
  // Shared by the public site, (site), and the portal, /portal. Each group adds its own chrome.
  import "../app.css";
  import "#lib/portal/ui/tokens.css";
  import { onMount, type Snippet } from "svelte";
  import { browser } from "$app/env";
  import { beforeNavigate } from "$app/navigation";
  import { page } from "$app/state";
  import { PUBLIC_POSTHOG_HOST, PUBLIC_POSTHOG_KEY } from "$app/env/public";
  import satoshiFont from "#lib/assets/fonts/Satoshi-Regular.woff2";
  import satoshiFontBold from "#lib/assets/fonts/Satoshi-Bold.woff2";
  import satoshiFontMedium from "#lib/assets/fonts/Satoshi-Medium.woff2";
  import satoshiFontBlack from "#lib/assets/fonts/Satoshi-Black.woff2";
  import {
    analyticsOptions,
    analyticsSettings,
    asksNotToBeTracked,
    opensInFull,
    type TrackingPreferences,
  } from "#lib/site/analytics.ts";
  import { removeInjectedHeadTags } from "#lib/site/head.ts";

  let { children }: { children: Snippet } = $props();

  // A project page's shell arrives with the service's tags for crawlers. They go before any page
  // writes its own head, so there is one description and it follows client-side navigation.
  if (browser) removeInjectedHeadTags(document.head);

  // Page views on public pages only (src/lib/site/analytics.ts). PostHog loads after the page, in
  // its own chunk, so it never holds up the content, and never in a page load that starts in the
  // portal or for a visitor who asked not to be tracked.
  let analyticsLoaded = false;

  onMount(() => {
    const settings = analyticsSettings({
      key: PUBLIC_POSTHOG_KEY,
      host: PUBLIC_POSTHOG_HOST,
      pathname: location.pathname,
      doNotTrack: asksNotToBeTracked(navigator as TrackingPreferences),
    });
    if (settings === null) return;
    analyticsLoaded = true;
    void import("posthog-js").then(({ default: posthog }) => {
      posthog.init(settings.key, analyticsOptions(settings.host));
    });
  });

  // With PostHog in the page, the portal opens in a page load of its own, without it. Going back
  // into the portal through history cannot happen in such a page; its events would be dropped.
  beforeNavigate((navigation) => {
    const to = navigation.to?.url;
    if (to === undefined || navigation.willUnload || navigation.type === "popstate") return;
    const route = navigation.to?.route.id ?? null;
    if (!opensInFull(analyticsLoaded, new URL(location.href), to, route)) return;
    navigation.cancel();
    location.assign(to.href);
  });
</script>

<svelte:head>
  <title>ScottyLabs</title>
  <!-- A page that writes its own description returns { ownDescription: true } from its load. -->
  {#if page.data.ownDescription !== true}
    <meta name="description" content="The landing page for Scottylabs" />
  {/if}
  <!-- The text faces only. The monospace face is small and loads when a page uses it. -->
  {#each [satoshiFont, satoshiFontBold, satoshiFontMedium, satoshiFontBlack] as font (font)}
    <link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
  {/each}
</svelte:head>

{@render children()}
