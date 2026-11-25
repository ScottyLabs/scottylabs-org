<script lang="ts">
  // The public site's header and footer around a page. Used by the (site) layout and by the root
  // error page, which renders outside the (site) group for addresses that match no page.
  import type { Snippet } from "svelte";
  import Header from "./Header.svelte";
  import Footer from "./Footer.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<div class="layout">
  <!-- Keyboard users skip the logo and the navigation, as in the portal. -->
  <a class="skip" href="#site-main">Skip to content</a>
  <Header />
  <main id="site-main" tabindex="-1">
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

    &:focus {
      outline: none;
    }
  }
  .skip {
    position: fixed;
    top: 8px;
    left: 8px;
    z-index: 100;
    padding: 10px 16px;
    border-radius: var(--p-radius);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-family: var(--p-font);
    font-size: 0.875rem;
    font-weight: 500;
    text-decoration: none;
    transform: translateY(-200%);

    &:focus-visible {
      transform: none;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
</style>
