<script lang="ts">
  // Sign in as a seeded person without Keycloak. The service only allows it on local and preview
  // deployments (contract, "Dev login rules"); this panel only appears when it says so. On
  // previews the service lists the people only once it has the key, so the key comes first.
  import { tick } from "svelte";
  import FlaskConical from "@lucide/svelte/icons/flask-conical";
  import { isApiError, post } from "../../api/client.ts";
  import type {
    AuthConfigResponse,
    DevLoginInput,
    DevPersona,
    DevPersonasInput,
    DevPersonasResponse,
  } from "../../api/types.ts";
  import Avatar from "../ui/Avatar.svelte";
  import Button from "../ui/Button.svelte";
  import Callout from "../ui/Callout.svelte";
  import Field from "../ui/Field.svelte";
  import Spinner from "../ui/Spinner.svelte";
  import TextInput from "../ui/TextInput.svelte";

  let {
    personas,
    needsKey,
    onsignedin,
  }: {
    personas: AuthConfigResponse["devPersonas"];
    needsKey: boolean;
    /**
     * Called after the service accepted the sign-in. Takes the person onward and resolves true, or
     * resolves false when no session came of it.
     */
    onsignedin: () => Promise<boolean>;
  } = $props();

  let key = $state("");
  let pending = $state<string | null>(null);
  let error = $state<string | null>(null);
  let keyError = $state<string | null>(null);
  /** The people the service listed once it had the key. */
  let unlocked = $state.raw<DevPersona[] | null>(null);
  let unlocking = $state(false);
  let list = $state<HTMLUListElement | null>(null);

  const shown = $derived(unlocked ?? personas);
  const needsUnlock = $derived(needsKey && shown.length === 0);

  /** Sends the key and, when the service accepts it, lists the people and focuses the first. */
  async function unlock(event: SubmitEvent) {
    event.preventDefault();
    if (unlocking || pending !== null) return;
    error = null;
    keyError = null;
    if (key.trim() === "") {
      keyError = "Enter the development access key first.";
      return;
    }
    unlocking = true;
    try {
      const body: DevPersonasInput = { key: key.trim() };
      const response = await post<DevPersonasResponse>("/auth/dev-personas", body, {
        redirectOnUnauthenticated: false,
      });
      unlocked = response.personas;
    } catch (caught) {
      keyError = isApiError(caught)
        ? caught.message
        : "We could not check the key. Try again in a moment.";
      return;
    } finally {
      unlocking = false;
    }
    await tick();
    list?.querySelector("button")?.focus();
  }

  function initials(name: string): string {
    const parts = name.trim().split(/\s+/u);
    return ((parts[0]?.[0] ?? "") + (parts.length > 1 ? (parts.at(-1)?.[0] ?? "") : "")) || "?";
  }

  async function signIn(andrewId: string) {
    if (pending !== null) return;
    error = null;
    keyError = null;
    if (needsKey && key.trim() === "") {
      keyError = "Enter the development access key first.";
      return;
    }
    pending = andrewId;
    try {
      const body: DevLoginInput = { andrewId, key: needsKey ? key.trim() : null };
      await post("/auth/dev-login", body, { redirectOnUnauthenticated: false });
    } catch (caught) {
      error = isApiError(caught)
        ? caught.message
        : "We could not sign you in. Try again in a moment.";
      pending = null;
      return;
    }
    // Signed in. Keep the spinner while the page moves on; only report what went wrong after.
    error = await continueAfterSignIn();
    if (error !== null) pending = null;
  }

  async function continueAfterSignIn(): Promise<string | null> {
    try {
      if (await onsignedin()) return null;
      return "You signed in, but this browser did not keep the session. Allow cookies for this site and try again.";
    } catch {
      return "You signed in, but we could not open the portal. Reload the page to continue.";
    }
  }
</script>

<section class="dev" aria-labelledby="dev-sign-in-title">
  <div class="dev__header">
    <span class="dev__icon"><FlaskConical size={16} strokeWidth={1.75} aria-hidden="true" /></span>
    <div>
      <h2 id="dev-sign-in-title" class="dev__title">Development sign-in</h2>
      <p class="dev__text">
        This copy of the site lets you sign in as a sample person to try the portal. It is never
        available on scottylabs.org.
      </p>
    </div>
  </div>

  {#if needsKey}
    <form class="dev__key" onsubmit={unlock}>
      <Field
        label="Development access key"
        help="Ask whoever set up this preview for the key."
        error={keyError}
      >
        <TextInput bind:value={key} type="password" autocomplete="off" spellcheck={false} />
      </Field>
      {#if needsUnlock}
        <Button type="submit" variant="outline" loading={unlocking}>Show sample people</Button>
      {/if}
    </form>
  {/if}

  {#if error}
    <Callout tone="error" role="alert">{error}</Callout>
  {/if}

  {#if shown.length > 0}
    <ul class="dev__list" aria-label="Sample people" bind:this={list}>
      {#each shown as persona (persona.andrewId)}
        <li>
          <button
            type="button"
            class="dev__persona"
            disabled={pending !== null}
            aria-busy={pending === persona.andrewId ? "true" : undefined}
            onclick={() => signIn(persona.andrewId)}
          >
            <Avatar initials={initials(persona.name)} />
            <span class="dev__who">
              <span class="dev__name">Sign in as {persona.name}</span>
              <span class="dev__label">{persona.label}</span>
            </span>
            {#if pending === persona.andrewId}
              <Spinner size={14} label="Signing in" />
            {:else}
              <span class="dev__id">{persona.andrewId}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .dev {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 20px;
    border: 1px dashed #c9a462;
    border-radius: var(--p-radius-lg);
    background: #fffaf0;
  }
  .dev__header {
    display: flex;
    gap: 12px;
  }
  .dev__icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--p-warning-surface);
    color: var(--p-warning-text);
  }
  .dev__title {
    margin: 0;
    color: var(--p-text);
    font-size: 0.9375rem;
    font-weight: 700;
    line-height: 22px;
  }
  .dev__text {
    margin: 2px 0 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 19px;
  }
  .dev__key {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;

    & > :global(*:first-child) {
      align-self: stretch;
    }
  }
  .dev__list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dev__persona {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 52px;
    padding: 8px 14px 8px 10px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    cursor: pointer;
    transition:
      border-color var(--p-duration) var(--p-ease),
      background-color var(--p-duration) var(--p-ease);

    &:hover:not(:disabled) {
      border-color: var(--p-text);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      cursor: progress;
    }
  }
  .dev__who {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
    text-align: left;
  }
  .dev__name {
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .dev__label {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .dev__id {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
  }
</style>
