<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Info from "@lucide/svelte/icons/info";
  import X from "@lucide/svelte/icons/x";
  import type { ToastStore } from "../toasts.svelte.ts";

  let { store }: { store: ToastStore } = $props();

  const icons = { success: Check, error: CircleAlert, info: Info } as const;
</script>

<div class="p-toasts" role="region" aria-label="Notifications">
  <ol class="p-toasts__list" aria-live="polite" aria-relevant="additions text">
    {#each store.toasts as toast (toast.id)}
      {@const Icon = icons[toast.tone]}
      <li
        class={["p-toast", `p-toast--${toast.tone}`]}
        onpointerenter={() => store.pause(toast.id)}
        onpointerleave={() => store.resume(toast.id)}
        onfocusin={() => store.pause(toast.id)}
        onfocusout={() => store.resume(toast.id)}
      >
        <Icon size={16} strokeWidth={2} aria-hidden="true" />
        <span class="p-toast__message">{toast.message}</span>
        {#if toast.action}
          <a
            class="p-toast__action"
            href={toast.action.href}
            target={toast.action.external ? "_blank" : undefined}
            rel={toast.action.external ? "noopener noreferrer" : undefined}
            onclick={() => store.dismiss(toast.id)}
          >
            {toast.action.label}{#if toast.action.external}<span class="p-sr-only"> (opens in a new tab)</span>{/if}
          </a>
        {/if}
        <button
          type="button"
          class="p-toast__close"
          aria-label="Dismiss notification"
          onclick={() => store.dismiss(toast.id)}
        >
          <X size={14} strokeWidth={2} aria-hidden="true" />
        </button>
      </li>
    {/each}
  </ol>
</div>

<style>
  .p-toasts {
    position: fixed;
    left: 50%;
    bottom: calc(var(--p-toast-offset, 24px) + env(safe-area-inset-bottom));
    z-index: 1000;
    width: max-content;
    max-width: calc(100vw - 32px);
    transform: translateX(-50%);
    pointer-events: none;
  }
  .p-toasts__list {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .p-toast {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 100%;
    min-height: 40px;
    padding: 8px 8px 8px 16px;
    border-radius: 20px;
    background: var(--p-toast);
    color: #ffffff;
    box-shadow: var(--p-shadow-pop);
    font-family: var(--p-font);
    font-size: 0.875rem;
    line-height: 20px;
    pointer-events: auto;
    animation: toast-in 200ms var(--p-ease);

    & > :global(svg) {
      flex: none;
    }
  }
  .p-toast--error > :global(svg:first-child) {
    color: #ff9b9b;
  }
  .p-toast__message {
    min-width: 0;
  }
  .p-toast__action {
    flex: none;
    color: #ffffff;
    font-weight: 500;
    text-decoration: underline;
    text-underline-offset: 3px;
    border-radius: 4px;

    &:hover {
      text-decoration-thickness: 2px;
    }
    &:focus-visible {
      outline: 2px solid #ffffff;
      outline-offset: 2px;
    }
  }
  .p-toast__close {
    display: grid;
    place-items: center;
    flex: none;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    color: #d4d4d4;
    cursor: pointer;

    &:hover {
      background: rgba(255, 255, 255, 0.14);
      color: #ffffff;
    }
    &:focus-visible {
      outline: 2px solid #ffffff;
      outline-offset: 1px;
    }
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .p-toast {
      animation: none;
    }
  }
</style>
