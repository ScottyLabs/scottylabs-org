<script lang="ts">
  // Shared by the public site, (site), and the portal, /portal. Each group adds its own chrome.
  import "../app.css";
  import "#lib/portal/ui/tokens.css";
  import { onMount, type Snippet } from "svelte";
  import posthog from "posthog-js";
  import { browser } from "$app/env";
  import { page } from "$app/state";
  import { PUBLIC_POSTHOG_HOST, PUBLIC_POSTHOG_KEY } from "$app/env/public";
  import satoshiFont from "#lib/assets/fonts/Satoshi-Regular.woff2";
  import satoshiFontBold from "#lib/assets/fonts/Satoshi-Bold.woff2";
  import satoshiFontMedium from "#lib/assets/fonts/Satoshi-Medium.woff2";
  import satoshiFontBlack from "#lib/assets/fonts/Satoshi-Black.woff2";
  import jetbrainsMono from "#lib/assets/fonts/JetBrainsMono-Regular.ttf";
  import { removeInjectedHeadTags } from "#lib/site/head.ts";

  let { children }: { children: Snippet } = $props();

  // A project page's shell arrives with the service's tags for crawlers. They go before any page
  // writes its own head, so there is one description and it follows client-side navigation.
  if (browser) removeInjectedHeadTags(document.head);

  onMount(() => {
    if (PUBLIC_POSTHOG_KEY === undefined) return;
    posthog.init(PUBLIC_POSTHOG_KEY, {
      api_host: PUBLIC_POSTHOG_HOST,
      ui_host: "https://us.posthog.com",
      defaults: "2025-05-24",
    });
  });
</script>

<svelte:head>
  <title>ScottyLabs</title>
  <!-- A page that writes its own description returns { ownDescription: true } from its load. -->
  {#if page.data.ownDescription !== true}
    <meta name="description" content="The landing page for Scottylabs" />
  {/if}
  {#each [satoshiFont, satoshiFontBold, satoshiFontMedium, satoshiFontBlack, jetbrainsMono] as font (font)}
    <link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
  {/each}
</svelte:head>

{@render children()}
