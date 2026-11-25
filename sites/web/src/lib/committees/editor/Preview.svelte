<script lang="ts">
  // Preview (Figma 109:9435, spec CP-8): the draft exactly as visitors will see it, at desktop or
  // phone width. A full-screen modal dialog, so the editor behind it is inert. Escape or "Back to
  // editing" returns to the editor, and focus goes back to what opened the preview.
  import Monitor from "@lucide/svelte/icons/monitor";
  import Smartphone from "@lucide/svelte/icons/smartphone";
  import { Button } from "../../portal/ui/index.ts";
  import CommitteePage from "../CommitteePage.svelte";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    device = $bindable("desktop"),
    onclose,
    onpublish,
  }: {
    editor: PageEditor;
    device?: "desktop" | "phone";
    onclose: () => void;
    onpublish: () => void;
  } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let closed = false;

  $effect(() => {
    if (dialog === undefined) return;
    const opener = document.activeElement;
    const element = dialog;
    element.showModal();
    element.querySelector<HTMLElement>("[data-preview-back]")?.focus();
    return () => {
      closed = true;
      if (element.open) element.close();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
    };
  });

  // Escape closes the dialog by itself, and Chrome does not always let that be cancelled, so the
  // editor hears about every close, however it happened.
  function handleClose() {
    if (!closed) onclose();
  }
</script>

<dialog class="preview" bind:this={dialog} aria-label="Preview" onclose={handleClose}>
  <div class="preview__bar">
    <p class="preview__note">Preview. This is what visitors will see once you publish.</p>
    <div class="preview__device" role="group" aria-label="Preview width">
      <button type="button" aria-pressed={device === "desktop"} onclick={() => (device = "desktop")}>
        <Monitor size={16} strokeWidth={1.75} aria-hidden="true" />Desktop
      </button>
      <button type="button" aria-pressed={device === "phone"} onclick={() => (device = "phone")}>
        <Smartphone size={16} strokeWidth={1.75} aria-hidden="true" />Phone
      </button>
    </div>
    <div class="preview__actions">
      <Button variant="outline" size="sm" data-preview-back onclick={onclose}>Back to editing</Button>
      {#if editor.writable}<Button size="sm" onclick={onpublish}>Publish</Button>{/if}
    </div>
  </div>
  <div class="preview__scroll">
    <div class={["preview__page", `preview__page--${device}`]}>
      <CommitteePage
        committee={editor.committee}
        settings={editor.doc.settings}
        blocks={editor.doc.blocks}
        projects={editor.projects}
      />
    </div>
  </div>
</dialog>

<style>
  .preview {
    box-sizing: border-box;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    padding: 0;
    border: 0;
    background: var(--p-bg);
    color: var(--p-text);
    font-family: var(--p-font);

    &[open] {
      display: flex;
      flex-direction: column;
    }
    &::backdrop {
      background: transparent;
    }
    &:focus-visible {
      outline: none;
    }
  }
  .preview__bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 24px;
    padding: 10px 20px;
    border-bottom: 1px solid #b9dcf8;
    background: var(--p-info-surface);
  }
  .preview__note {
    flex: 1;
    margin: 0;
    color: var(--p-info-text);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .preview__device {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--p-radius);
    background: #ffffff;

    & button {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 5px 10px;
      border-radius: var(--p-radius-sm);
      color: var(--p-text-secondary);
      font-size: 0.8125rem;
      line-height: 18px;
      cursor: pointer;
    }
    & button[aria-pressed="true"] {
      background: var(--p-inverse);
      color: var(--p-on-inverse);
    }
    & button:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }
  .preview__actions {
    display: flex;
    gap: 8px;
  }
  .preview__scroll {
    flex: 1;
    overflow: auto;
  }
  .preview__page {
    box-sizing: border-box;
    margin: 0 auto;
    background: #ffffff;
  }
  .preview__page--desktop {
    max-width: 1280px;
    min-height: 100%;
    padding: 36px 40px 96px;
  }
  .preview__page--phone {
    width: 390px;
    margin-block: 24px;
    padding: 24px 20px 48px;
    border: 1px solid var(--p-border);
    border-radius: 32px;
  }
</style>
