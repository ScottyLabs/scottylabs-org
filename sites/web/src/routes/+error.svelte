<script lang="ts">
  // Errors from public pages, and addresses that match no page anywhere. The portal has its own
  // error page for errors inside it; unknown portal addresses land here and get the portal style.
  import { page } from "$app/state";
  import errorRyo from "#lib/assets/ryo.webp";
  import SiteFrame from "#lib/sections/home/SiteFrame.svelte";
  import PortalError from "#lib/portal/PortalError.svelte";

  const inPortal = $derived(
    page.url.pathname === "/portal" || page.url.pathname.startsWith("/portal/"),
  );
  const message = $derived(page.status === 404 ? "404" : "Error");
  const details = $derived(
    page.status === 404
      ? "The requested page could not be found."
      : (page.error?.message ?? "An unexpected error occurred."),
  );
</script>

{#if inPortal}
  <PortalError status={page.status} message={page.error?.message} standalone />
{:else}
  <SiteFrame>
    <div class="error-container">
      <h1 class="error-message">{message}</h1>
      <p class="error-details">{details}</p>
      <img src={errorRyo} height={400} alt="" />
      <br />
      <a href="/" class="error-go-home">
        <button>Go home</button>
      </a>
    </div>
  </SiteFrame>
{/if}

<style>
  .error-container {
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
    font-size: 1.5rem;
  }
</style>
