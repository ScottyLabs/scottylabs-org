<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Lock from "@lucide/svelte/icons/lock";
  import LogOut from "@lucide/svelte/icons/log-out";
  import { isApiError } from "../../api/client.ts";
  import { canSeePeople, describeAuthMode, describeRoles } from "../roles.ts";
  import { getPortalSession } from "../session.svelte.ts";
  import { getToasts } from "../toasts.svelte.ts";
  import Avatar from "../ui/Avatar.svelte";
  import Menu from "../ui/Menu.svelte";
  import type { MenuItem } from "../ui/types.ts";

  let {
    compact = false,
  }: {
    /** Phones: the avatar alone, and View site in the menu. */
    compact?: boolean;
  } = $props();

  const session = getPortalSession();
  const toasts = getToasts();

  // Read without throwing: signing out clears the session a moment before this unmounts.
  const me = $derived(session.me);
  const tint = $derived(
    me?.roles.find((role) => role.role !== "admin" && role.committee !== null)?.committee ?? null,
  );

  async function signOut() {
    try {
      await session.signOut();
    } catch (error) {
      toasts.error(
        isApiError(error) ? error.message : "We could not sign you out. Try again in a moment.",
      );
    }
  }

  const items = $derived.by(() => {
    const list: (MenuItem | "separator")[] = [];
    if (me !== null && me.permissions.canUsePortal && canSeePeople(me.permissions)) {
      list.push({ label: "Your role and access", icon: Lock, href: "/portal/people/" });
    }
    if (compact) {
      list.push({ label: "View site", icon: ExternalLink, href: "/", external: true });
    }
    list.push({ label: "Sign out", icon: LogOut, onSelect: () => void signOut() });
    return list;
  });
</script>

{#if me !== null}
<Menu {items} label="Account" width={280}>
  {#snippet header()}
    <p class="account__name">{me.user.name}</p>
    {#if me.roles.length > 0}
      <p class="account__line">{describeRoles(me.roles)}</p>
    {:else}
      <p class="account__line">No role yet</p>
    {/if}
    <p class="account__line account__line--muted">{describeAuthMode(me.authMode)}</p>
  {/snippet}
  {#snippet trigger(props)}
    <button
      {...props}
      type="button"
      class={["account__trigger", compact && "account__trigger--compact"]}
      aria-label={compact ? `Account menu for ${me.user.name}` : undefined}
    >
      <Avatar initials={me.user.initials} committee={tint} />
      {#if !compact}
        <span class="account__trigger-name">{me.user.name}</span>
        <ChevronDown size={14} strokeWidth={2} aria-hidden="true" />
      {/if}
    </button>
  {/snippet}
</Menu>
{/if}

<style>
  .account__trigger {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    max-width: 240px;
    padding: 4px 12px 4px 4px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-pill);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover,
    &[aria-expanded="true"] {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .account__trigger--compact {
    width: 38px;
    padding: 4px;
    border-color: transparent;
    justify-content: center;
  }
  .account__trigger-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .account__name {
    margin: 0;
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 18px;
  }
  .account__line {
    margin: 2px 0 0;
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .account__line--muted {
    color: var(--p-text-tertiary);
  }
</style>
