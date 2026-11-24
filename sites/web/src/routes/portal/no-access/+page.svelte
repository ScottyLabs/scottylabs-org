<script lang="ts">
  // For people who signed in but have no role yet (spec S-4).
  import Lock from "@lucide/svelte/icons/lock";
  import LogOut from "@lucide/svelte/icons/log-out";
  import { isApiError } from "#lib/api/client.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import { getToasts } from "#lib/portal/toasts.svelte.ts";
  import { Button, Card } from "#lib/portal/ui/index.ts";

  const session = getPortalSession();
  const toasts = getToasts();
  const me = $derived(session.me);

  let signingOut = $state(false);

  async function signOut() {
    signingOut = true;
    try {
      await session.signOut();
    } catch (error) {
      toasts.error(
        isApiError(error) ? error.message : "We could not sign you out. Try again in a moment.",
      );
    } finally {
      signingOut = false;
    }
  }
</script>

<svelte:head>
  <title>Nothing to edit yet - ScottyLabs Portal</title>
</svelte:head>

<div class="no-access">
  <Card padding="lg" class="no-access__card">
    <span class="no-access__icon"><Lock size={20} strokeWidth={1.75} aria-hidden="true" /></span>
    <h1 class="no-access__title" tabindex="-1">
      You are signed in, but there is nothing for you to edit yet.
    </h1>
    <p class="no-access__body">
      Editing is for committee leads and directors. Ask the director of your committee to add you,
      and it will show up here the next time you sign in.
    </p>
    {#if me}
      <p class="no-access__who">
        Signed in as {me.user.name} <span class="no-access__id">({me.user.andrewId})</span>
      </p>
    {/if}
    <div class="no-access__actions">
      <Button href="/team/">See who the directors are</Button>
      <Button variant="outline" icon={LogOut} loading={signingOut} onclick={signOut}>Sign out</Button>
    </div>
  </Card>
</div>

<style>
  .no-access {
    display: flex;
    justify-content: center;
    padding-top: clamp(24px, 14vh, 150px);
  }
  .no-access :global(.no-access__card) {
    display: flex;
    flex-direction: column;
    gap: 18px;
    width: 100%;
    max-width: 520px;
  }
  .no-access__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--p-surface-muted);
  }
  .no-access__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
    &:focus {
      outline: none;
    }
  }
  .no-access__body {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .no-access__who {
    margin: -6px 0 0;
    color: var(--p-text-tertiary);
    font-size: 0.8125rem;
    line-height: 20px;
  }
  .no-access__id {
    font-family: var(--p-mono);
    font-size: 0.75rem;
  }
  .no-access__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding-top: 6px;
  }
</style>
