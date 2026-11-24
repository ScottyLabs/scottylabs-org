<script lang="ts">
  // Shown when GET /api/me fails for a reason other than being signed out, such as no connection.
  import CloudOff from "@lucide/svelte/icons/cloud-off";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import type { ApiError } from "../../api/client.ts";
  import Button from "../ui/Button.svelte";
  import Card from "../ui/Card.svelte";

  let { error, onretry }: { error: ApiError | null; onretry: () => void } = $props();
</script>

<svelte:head>
  <title>Could not check your access - ScottyLabs Portal</title>
</svelte:head>

<main class="session-error">
  <Card padding="lg" class="session-error__card">
    <span class="session-error__icon"><CloudOff size={20} strokeWidth={1.75} aria-hidden="true" /></span>
    <h1 class="session-error__title">We could not check what you can edit</h1>
    <p class="session-error__body" role="alert">
      {error?.message ?? "Something went wrong on our side. Try again in a moment."}
    </p>
    <div class="session-error__actions">
      <Button icon={RefreshCw} onclick={onretry}>Try again</Button>
      <Button variant="outline" href="/">Go to scottylabs.org</Button>
    </div>
  </Card>
</main>

<style>
  .session-error {
    display: flex;
    justify-content: center;
    box-sizing: border-box;
    min-height: 100vh;
    padding: 18vh 16px 48px;
    background: var(--p-bg);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .session-error :global(.session-error__card) {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
    max-width: 520px;
    height: fit-content;
  }
  .session-error__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--p-surface-muted);
  }
  .session-error__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .session-error__body {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .session-error__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding-top: 4px;
  }
</style>
