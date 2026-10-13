<script lang="ts">
  import { page } from "$app/state";
  import errorRyo from "#lib/assets/ryo.webp";

  const message = $derived(page.status === 404 ? "404" : "Error");
  const details = $derived(
    page.status === 404
      ? "The requested page could not be found."
      : (page.error?.message ?? "An unexpected error occurred."),
  );
</script>

<div class="error-container">
  <h1 class="error-message">{message}</h1>
  <p class="error-details">{details}</p>
  <img src={errorRyo} height={400} alt="" />
  <br />
  <a href="/" class="error-go-home">
    <button>Go home</button>
  </a>
</div>

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
