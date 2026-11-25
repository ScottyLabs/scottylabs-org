<script lang="ts">
  // Shared by the public site, (site), and the portal, /portal. Each group adds its own chrome.
  import "../app.css";
  import "#lib/portal/ui/tokens.css";
  import { onMount, type Snippet } from "svelte";
  import { browser } from "$app/env";
  import { page } from "$app/state";
  import { PUBLIC_POSTHOG_HOST, PUBLIC_POSTHOG_KEY } from "$app/env/public";
  import satoshiFont from "#lib/assets/fonts/Satoshi-Regular.woff2";
  import satoshiFontBold from "#lib/assets/fonts/Satoshi-Bold.woff2";
  import satoshiFontMedium from "#lib/assets/fonts/Satoshi-Medium.woff2";
  import satoshiFontBlack from "#lib/assets/fonts/Satoshi-Black.woff2";
  import { removeInjectedHeadTags } from "#lib/site/head.ts";

  let { children }: { children: Snippet } = $props();

  // A project page's shell arrives with the service's tags for crawlers. They go before any page
  // writes its own head, so there is one description and it follows client-side navigation.
  if (browser) removeInjectedHeadTags(document.head);

  // Analytics load after the page, in their own chunk, so they never hold up its content.
  onMount(() => {
    const key = PUBLIC_POSTHOG_KEY;
    if (key === undefined) return;
    void import("posthog-js").then(({ default: posthog }) => {
      posthog.init(key, {
        api_host: PUBLIC_POSTHOG_HOST,
        ui_host: "https://us.posthog.com",
        defaults: "2025-05-24",
      });
    });
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
