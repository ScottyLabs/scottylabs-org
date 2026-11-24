<script lang="ts">
  // Shared by Dialog and Sheet. A native modal <dialog> keeps focus inside and makes the page
  // behind it inert; this adds Escape and backdrop rules, scroll locking and returning focus.
  import type { Snippet } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import IconButton from "./IconButton.svelte";
  import { lockScroll } from "./scroll-lock.ts";

  let {
    open = $bindable(false),
    variant,
    title,
    description,
    size = "md",
    dismissible = true,
    hideTitle = false,
    onclose,
    children,
    footer,
    class: className,
  }: {
    open?: boolean;
    variant: "dialog" | "sheet";
    title: string;
    description?: string;
    size?: "sm" | "md" | "lg";
    /** When false, Escape, the backdrop and the close button do nothing, for example while saving. */
    dismissible?: boolean;
    hideTitle?: boolean;
    onclose?: () => void;
    children?: Snippet;
    /** Actions, right aligned under the content. */
    footer?: Snippet;
    class?: string;
  } = $props();

  const uid = $props.id();
  const titleId = `modal-title-${uid}`;
  const descriptionId = `modal-description-${uid}`;

  let dialog: HTMLDialogElement | undefined = $state();
  let returnFocusTo: HTMLElement | null = null;
  let unlock: (() => void) | null = null;
  let pressedOnBackdrop = false;

  $effect(() => {
    if (dialog === undefined) return;
    if (open && !dialog.open) {
      const active = document.activeElement;
      returnFocusTo = active instanceof HTMLElement ? active : null;
      unlock = lockScroll();
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  });

  // Leaving the page with the dialog open must not leave the page locked.
  $effect(() => () => {
    unlock?.();
    unlock = null;
  });

  // The cancel event alone cannot hold the dialog open: Chrome makes it non-cancelable when there
  // was no click or key press since the dialog opened. Stopping Escape itself prevents the close
  // request. Listening on the document also covers focus resting on the body.
  $effect(() => {
    if (!open || dismissible) return;
    document.addEventListener("keydown", holdEscape, { capture: true });
    return () => document.removeEventListener("keydown", holdEscape, { capture: true });
  });

  function holdEscape(event: KeyboardEvent) {
    if (event.key === "Escape") event.preventDefault();
  }

  function handleClose() {
    // Something closed the dialog while it must stay open, such as a close request that got past
    // the guards. Open it again rather than drop a save in progress.
    if (open && !dismissible && dialog !== undefined) {
      dialog.showModal();
      return;
    }
    unlock?.();
    unlock = null;
    open = false;
    onclose?.();
    const target = returnFocusTo;
    returnFocusTo = null;
    if (target?.isConnected) target.focus();
  }

  function handleCancel(event: Event) {
    if (!dismissible) event.preventDefault();
  }

  function handlePointerDown(event: PointerEvent) {
    pressedOnBackdrop = event.target === dialog;
  }

  function handleClick(event: MouseEvent) {
    // The surface fills the dialog box, so only clicks on the backdrop land on the dialog itself.
    if (dismissible && pressedOnBackdrop && event.target === dialog) open = false;
    pressedOnBackdrop = false;
  }
</script>

<dialog
  bind:this={dialog}
  class={["p-modal", `p-modal--${variant}`, `p-modal--${size}`, className]}
  aria-modal="true"
  aria-labelledby={titleId}
  aria-describedby={description ? descriptionId : undefined}
  onclose={handleClose}
  oncancel={handleCancel}
  onpointerdown={handlePointerDown}
  onclick={handleClick}
>
  {#if open}
    <div class="p-modal__surface">
      {#if variant === "sheet"}<span class="p-modal__handle" aria-hidden="true"></span>{/if}
      <header class={["p-modal__header", hideTitle && "p-sr-only"]}>
        <h2 id={titleId} class="p-modal__title">{title}</h2>
        {#if description}<p id={descriptionId} class="p-modal__description">{description}</p>{/if}
      </header>
      {#if children}<div class="p-modal__body">{@render children()}</div>{/if}
      {#if footer}<footer class="p-modal__footer">{@render footer()}</footer>{/if}
      {#if dismissible}
        <IconButton class="p-modal__close" label="Close" icon={X} size="sm" onclick={() => (open = false)} />
      {/if}
    </div>
  {/if}
</dialog>

<style>
  .p-modal {
    box-sizing: border-box;
    padding: 0;
    border: 1px solid var(--p-border);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    box-shadow: var(--p-shadow-dialog);
    overflow: hidden;

    &::backdrop {
      background: rgba(0, 0, 0, 0.45);
    }
    &:focus-visible {
      outline: none;
    }
  }
  .p-modal[open] {
    display: flex;
  }

  .p-modal--dialog {
    width: min(var(--modal-width), calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    border-radius: var(--p-radius-xl);
  }
  .p-modal--dialog[open] {
    animation: modal-in var(--p-duration) var(--p-ease);
  }
  .p-modal--sm {
    --modal-width: 420px;
  }
  .p-modal--md {
    --modal-width: 520px;
  }
  .p-modal--lg {
    --modal-width: 680px;
  }

  .p-modal--sheet {
    width: min(560px, 100vw);
    max-width: 100vw;
    max-height: 88dvh;
    margin: auto auto 0;
    border-bottom: 0;
    border-radius: 18px 18px 0 0;
  }
  .p-modal--sheet[open] {
    animation: sheet-in 220ms var(--p-ease);
  }

  .p-modal__surface {
    position: relative;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 20px;
    width: 100%;
    max-height: inherit;
    padding: 28px 28px 24px;
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .p-modal--sheet .p-modal__surface {
    padding: 10px 20px calc(20px + env(safe-area-inset-bottom));
  }
  @media (max-width: 600px) {
    .p-modal--dialog .p-modal__surface {
      padding: 22px 20px 20px;
    }
  }
  .p-modal__handle {
    align-self: center;
    width: 36px;
    height: 4px;
    margin-bottom: 4px;
    border-radius: 2px;
    background: var(--p-border);
  }
  .p-modal__header {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-right: 36px;
  }
  .p-modal__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .p-modal__description {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .p-modal__body {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .p-modal__footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
  .p-modal--sheet .p-modal__footer {
    flex-direction: column-reverse;
  }
  .p-modal--sheet .p-modal__footer > :global(*) {
    width: 100%;
  }
  .p-modal__surface > :global(.p-modal__close) {
    position: absolute;
    top: 22px;
    right: 20px;
  }
  .p-modal--sheet .p-modal__surface > :global(.p-modal__close) {
    top: 14px;
    right: 14px;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
  }
  @keyframes sheet-in {
    from {
      transform: translateY(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .p-modal--dialog[open],
    .p-modal--sheet[open] {
      animation: none;
    }
  }
</style>
