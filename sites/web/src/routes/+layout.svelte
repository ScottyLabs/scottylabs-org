<script lang="ts">
  import "../app.css";
  import { onMount, type Snippet } from "svelte";
  import posthog from "posthog-js";
  import { POSTHOG_HOST, POSTHOG_KEY } from "$app/env/public";
  import Header from "#lib/sections/home/Header.svelte";
  import Footer from "#lib/sections/home/Footer.svelte";
  import satoshiFont from "#lib/assets/fonts/Satoshi-Regular.woff2";
  import satoshiFontBold from "#lib/assets/fonts/Satoshi-Bold.woff2";
  import satoshiFontMedium from "#lib/assets/fonts/Satoshi-Medium.woff2";
  import satoshiFontBlack from "#lib/assets/fonts/Satoshi-Black.woff2";
  import jetbrainsMono from "#lib/assets/fonts/JetBrainsMono-Regular.ttf";

  let { children }: { children: Snippet } = $props();

  onMount(() => {
    if (POSTHOG_KEY === undefined) return;
    posthog.init(POSTHOG_KEY, {
      api_host: POSTHOG_HOST,
      ui_host: "https://us.posthog.com",
      defaults: "2025-05-24",
    });
  });
</script>

<svelte:head>
  <title>ScottyLabs</title>
  <meta name="description" content="The landing page for Scottylabs" />
  {#each [satoshiFont, satoshiFontBold, satoshiFontMedium, satoshiFontBlack, jetbrainsMono] as font (font)}
    <link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
  {/each}
</svelte:head>

<div class="layout">
  <Header />
  <main>
    {@render children()}
  </main>
  <Footer />
</div>

<style>
  .layout {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
  }
  main {
    flex-grow: 1;
  }
</style>
