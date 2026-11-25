<script lang="ts">
  // Errors from public pages, and addresses that match no page anywhere. The portal has its own
  // error page for errors inside it; unknown portal addresses land here and get the portal style.
  // SvelteKit loads this page with every page, so the portal's card (and the portal UI kit with
  // it) is loaded only when it is needed.
  import { page } from "$app/state";
  import errorRyo from "#lib/assets/ryo.webp";
  import SiteFrame from "#lib/sections/home/SiteFrame.svelte";

  const inPortal = $derived(
    page.url.pathname === "/portal" || page.url.pathname.startsWith("/portal/"),
  );
  const notFound = $derived(page.status === 404);
  const message = $derived(notFound ? "404" : "Error");
  const details = $derived(
    notFound
      ? "The requested page could not be found."
      : (page.error?.message ?? "An unexpected error occurred."),
  );
</script>

<svelte:head>
  <!-- The portal's card names its own page. -->
  {#if !inPortal}<title>{notFound ? "Page not found" : "Something went wrong"} - ScottyLabs</title>{/if}
</svelte:head>

{#if inPortal}
  {#await import("#lib/portal/PortalError.svelte") then { default: PortalError }}
    <PortalError status={page.status} message={page.error?.message} standalone />
  {/await}
{:else}
  <SiteFrame>
    <div class="error-container">
      <h1 class="error-message">{message}</h1>
      <p class="error-details">{details}</p>
      <img src={errorRyo} height={400} alt="" />
      <a href="/" class="error-go-home">Go home</a>
    </div>
  </SiteFrame>
{/if}

<style>
  .error-container {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 40px 10px;
    margin: auto;
    max-width: 1000px;
    color: var(--black-900);
    & img {
      margin-top: 40px;
    }
  }
  .error-message {
    font-size: 2.25rem;
  }
  .error-details {
    margin-top: 20px;
    font-size: 1.5rem;
  }
  .error-go-home {
    display: inline-flex;
    align-items: center;
    min-height: 48px;
    box-sizing: border-box;
    margin-top: 32px;
    padding: 12px 24px;
    border-radius: 100px;
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-size: 1rem;
    font-weight: 600;

    &:hover {
      background: var(--p-inverse-hover);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
</style>
