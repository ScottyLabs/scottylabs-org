<script lang="ts">
  // The public site's header and footer around a page. Used by the (site) layout and by the root
  // error page, which renders outside the (site) group for addresses that match no page.
  import type { Snippet } from "svelte";
  import Header from "./Header.svelte";
  import Footer from "./Footer.svelte";

  let { children }: { children: Snippet } = $props();
</script>

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
