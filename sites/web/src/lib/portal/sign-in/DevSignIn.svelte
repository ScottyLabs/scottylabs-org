<script lang="ts">
  // Sign in as a seeded person without Keycloak. The service only allows it on local and preview
  // deployments (contract, "Dev login rules"); this panel only appears when it says so.
  import FlaskConical from "@lucide/svelte/icons/flask-conical";
  import { isApiError, post } from "../../api/client.ts";
  import type { AuthConfigResponse, DevLoginInput } from "../../api/types.ts";
  import Avatar from "../ui/Avatar.svelte";
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
    /** Called after the session cookie is set. */
    onsignedin: () => Promise<void>;
  } = $props();

  let key = $state("");
  let pending = $state<string | null>(null);
  let error = $state<string | null>(null);
  let keyError = $state<string | null>(null);

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
      await onsignedin();
    } catch (caught) {
      error = isApiError(caught)
        ? caught.message
        : "We could not sign you in. Try again in a moment.";
    } finally {
      pending = null;
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
    <Field
      label="Development access key"
      help="Ask whoever set up this preview for the key."
      error={keyError}
    >
      <TextInput bind:value={key} type="password" autocomplete="off" spellcheck={false} />
    </Field>
  {/if}

  {#if error}
    <Callout tone="error" role="alert">{error}</Callout>
  {/if}

  <ul class="dev__list" aria-label="Sample people">
    {#each personas as persona (persona.andrewId)}
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
