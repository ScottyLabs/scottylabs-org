<script lang="ts">
  // The committee page editor (spec CP-2 to CP-12): full screen over the portal, with its own top
  // bar, the block library and outline on the left, the page in the middle and settings on the
  // right. On phones it becomes a list of blocks. Keyboard: Cmd or Ctrl+Z undoes, Shift with it
  // (or Ctrl+Y) redoes, Escape clears the selection, Alt with the arrows moves a block.
  import { onDestroy, onMount, tick } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import { isApiError } from "../../api/client.ts";
  import type { Block, CommitteePageEditorResponse } from "../../api/types.ts";
  import { getPortalSession, getToasts } from "../../portal/index.ts";
  import { Button, Callout, Dialog, lockScroll } from "../../portal/ui/index.ts";
  import { focusIsLost } from "../../portal/ui/focus.ts";
  import { discardChanges, loadEditor, releaseLockOnExit } from "../api.ts";
  import { MAX_BLOCKS, blockName } from "../blocks.ts";
  import { moveSentence } from "../layout.ts";
  import { savedLabel } from "../status.ts";
  import BlockLibrary from "./BlockLibrary.svelte";
  import Canvas from "./Canvas.svelte";
  import MobileEditor from "./MobileEditor.svelte";
  import Outline from "./Outline.svelte";
  import Preview from "./Preview.svelte";
  import PublishDialog from "./PublishDialog.svelte";
  import SettingsPanel from "./SettingsPanel.svelte";
  import TopBar from "./TopBar.svelte";
  import UndoToast, { type UndoNotice } from "./UndoToast.svelte";
  import type { EditorDrag } from "./drag.ts";
  import { inertOutside } from "./inert.ts";
  import { PageEditor } from "./state.svelte.ts";

  let { response }: { response: CommitteePageEditorResponse } = $props();

  // The editor owns its state from here on; a new page gets a new component.
  // svelte-ignore state_referenced_locally
  const editor = new PageEditor(response);
  const session = getPortalSession();
  const toasts = getToasts();
  const phone = new MediaQuery("(max-width: 899px)");

  let device = $state<"desktop" | "phone">("desktop");
  let previewDevice = $state<"desktop" | "phone">("desktop");
  let showGrid = $state(false);
  let drag = $state<EditorDrag | null>(null);
  let previewing = $state(false);
  let publishOpen = $state(false);
  let discardOpen = $state(false);
  let discarding = $state(false);
  let notice = $state<UndoNotice | null>(null);
  let noticeCount = 0;
  let checking = $state(false);
  let root: HTMLElement | undefined = $state();

  onMount(() => {
    const unlock = lockScroll();
    void editor.start();
    return unlock;
  });

  onDestroy(() => {
    // Saving and letting go of the lock carry on after the editor closes.
    void editor.leave();
  });

  function notify(message: string) {
    noticeCount += 1;
    notice = { id: noticeCount, message };
  }

  function onmoved(id: string) {
    notify(moveSentence(editor.doc.blocks, id));
  }

  function onremove(block: Block) {
    const index = editor.doc.blocks.findIndex((candidate) => candidate.id === block.id);
    if (editor.remove(block.id)) void onremoved(block, index);
  }

  async function onremoved(block: Block, index: number) {
    notify(`${blockName(block)} removed`);
    // The button that removed the block went with it. Keyboard and screen reader users carry on
    // from the block now in its place in the outline (on phones, its card), or the one before it
    // when the removed block was last, instead of the top of the document.
    await tick();
    if (!focusIsLost(undefined)) return;
    const targets = [...(root?.querySelectorAll<HTMLElement>(".outline__button, .mobile__toggle") ?? [])];
    // The cover cannot be removed, so there is always one to go to.
    targets[Math.min(index, targets.length - 1)]?.focus();
  }

  function undoFromNotice() {
    editor.undo();
    notice = null;
  }

  function addAtEnd(type: Parameters<PageEditor["addBlock"]>[0]) {
    const id = editor.addBlock(type);
    if (id !== null) requestAnimationFrame(() => reveal(id));
  }

  /** Scrolls a block into view on the canvas and selects it. */
  function reveal(blockId: string | null) {
    editor.select(blockId);
    if (blockId === null) return;
    const cell = document.querySelector(`[data-block-id="${CSS.escape(blockId)}"]`);
    cell?.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
  }

  function openPreview() {
    previewDevice = phone.current ? "phone" : device;
    previewing = true;
  }

  function openPublish() {
    previewing = false;
    publishOpen = true;
  }

  async function discard() {
    discarding = true;
    try {
      await editor.flush();
      editor.replace(await discardChanges(editor.slug));
      discardOpen = false;
      toasts.success("Changes discarded. The draft matches the published page again.");
    } catch (error) {
      if (!isApiError(error)) throw error;
      toasts.error(error.message);
    } finally {
      discarding = false;
    }
  }

  async function reload() {
    try {
      editor.replace(await loadEditor(editor.slug));
    } catch (error) {
      if (!isApiError(error)) throw error;
      toasts.error(error.message);
    }
  }

  async function checkAgain() {
    checking = true;
    try {
      await editor.retry();
    } finally {
      checking = false;
    }
  }

  function inField(target: EventTarget | null): boolean {
    return target instanceof HTMLElement && (target.isContentEditable || target.matches("input, textarea, select"));
  }

  function onkeydown(event: KeyboardEvent) {
    editor.noteActivity();
    if (previewing || publishOpen || discardOpen) return;
    const mod = event.metaKey || event.ctrlKey;
    const key = event.key.toLowerCase();
    if (mod && key === "z") {
      event.preventDefault();
      if (event.shiftKey) editor.redo();
      else editor.undo();
    } else if (event.ctrlKey && key === "y") {
      event.preventDefault();
      editor.redo();
    } else if (event.key === "Escape" && !inField(event.target) && editor.selectedId !== null) {
      const insideMenu = event.target instanceof Element && event.target.closest("[role='menu'], dialog") !== null;
      if (!insideMenu) editor.select(null);
    }
  }

  // Closing the tab with changes not yet saved asks first. The lock goes when the page does.
  function onbeforeunload(event: BeforeUnloadEvent) {
    if (editor.hasPendingChanges && editor.writable) event.preventDefault();
  }

  function onpagehide() {
    if (editor.lock?.mine === true) releaseLockOnExit(editor.slug, session.csrfToken);
  }

  const saveText = $derived(
    editor.saveState === "saved" && editor.savedAt !== null
      ? savedLabel(editor.savedAt, new Date())
      : editor.saveState === "saved"
        ? "All changes saved"
        : editor.saveState === "invalid" || editor.saveState === "conflict"
          ? "Not saved"
          : "Saving",
  );
  const firstProblem = $derived(Object.keys(editor.problems.blocks)[0] ?? null);
</script>

<svelte:window {onkeydown} {onbeforeunload} {onpagehide} onpointerdown={() => editor.noteActivity()} />

{#snippet notices()}
  {#if editor.readOnlyReason !== null && editor.canEdit}
    <Callout tone="warning" title="Someone else is editing this page" role="status">
      {editor.readOnlyReason} You are looking at the latest saved draft.
      {#snippet actions()}
        <Button variant="outline" size="sm" loading={checking} onclick={checkAgain}>Check again</Button>
      {/snippet}
    </Callout>
  {:else if !editor.canEdit}
    <Callout tone="info" title="You can look at this page">
      {editor.readOnlyReason} This is the draft as it was last saved.
    </Callout>
  {:else if editor.saveState === "conflict"}
    <Callout tone="error" title="Your changes are not saved" role="alert">
      {editor.saveMessage}
      {#snippet actions()}
        <Button variant="outline" size="sm" onclick={reload}>Reload the page</Button>
      {/snippet}
    </Callout>
  {:else if editor.saveState === "invalid"}
    <Callout tone="error" title="Your latest change is not saved" role="alert">
      {editor.saveMessage}
      {#snippet actions()}
        {#if firstProblem !== null}
          <Button variant="outline" size="sm" onclick={() => reveal(firstProblem)}>Show me</Button>
        {/if}
      {/snippet}
    </Callout>
  {/if}
{/snippet}

<div class="editor p-fields-compact" bind:this={root} {@attach inertOutside}>
  {#if phone.current}
    {@render notices()}
    <MobileEditor {editor} {saveText} onpreview={openPreview} onpublish={openPublish} {onremove} {onmoved} />
  {:else}
    <TopBar
      {editor}
      bind:device
      bind:showGrid
      onpreview={openPreview}
      onpublish={openPublish}
      ondiscard={() => (discardOpen = true)}
    />
    <div class="editor__body">
      <aside class="editor__left" aria-label="Blocks">
        {#if editor.writable}
          <BlockLibrary
            disabled={!editor.writable}
            full={editor.doc.blocks.length >= MAX_BLOCKS}
            onadd={addAtEnd}
            ondragchange={(next) => (drag = next)}
          />
          <hr class="editor__rule" />
        {/if}
        <Outline {editor} {onmoved} onselect={reveal} />
      </aside>
      <div class="editor__canvas">
        {#if editor.readOnlyReason !== null || editor.saveState === "conflict" || editor.saveState === "invalid"}
          <div class="editor__notice">{@render notices()}</div>
        {/if}
        <Canvas {editor} {device} {showGrid} bind:drag {onmoved} onremoved={(block, index) => void onremoved(block, index)} />
      </div>
      <aside class="editor__right" aria-label="Settings">
        <SettingsPanel {editor} {device} {onremove} />
      </aside>
    </div>
  {/if}
</div>

{#if previewing}
  <Preview {editor} bind:device={previewDevice} onclose={() => (previewing = false)} onpublish={openPublish} />
{/if}

<PublishDialog {editor} bind:open={publishOpen} onshow={reveal} />

<Dialog
  bind:open={discardOpen}
  title="Discard unpublished changes?"
  description="The draft goes back to the published page. Changes made since it was published are lost, and this cannot be undone."
  dismissible={!discarding}
  size="sm"
>
  {#snippet footer()}
    <Button variant="outline" disabled={discarding} onclick={() => (discardOpen = false)}>Keep editing</Button>
    <Button variant="danger" loading={discarding} onclick={discard}>Discard changes</Button>
  {/snippet}
</Dialog>

<UndoToast {notice} onundo={undoFromNotice} ondismiss={() => (notice = null)} />

<style>
  .editor {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    background: var(--p-bg);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .editor__body {
    display: grid;
    flex: 1;
    grid-template-columns: 272px minmax(0, 1fr) 312px;
    min-height: 0;
  }
  .editor__left,
  .editor__right {
    overflow-y: auto;
    background: var(--p-surface);
  }
  .editor__left {
    box-sizing: border-box;
    padding: 24px 16px 48px;
    border-right: 1px solid var(--p-border);
  }
  .editor__right {
    border-left: 1px solid var(--p-border);
  }
  .editor__rule {
    margin: 24px 0 20px;
    border: 0;
    border-top: 1px solid var(--p-border);
  }
  .editor__canvas {
    position: relative;
    overflow: auto;
    background: #e6e6e6;
  }
  .editor__notice {
    position: sticky;
    top: 0;
    z-index: 8;
    padding: 12px 36px 0;
  }

  @media (max-width: 1180px) {
    .editor__body {
      grid-template-columns: 232px minmax(0, 1fr) 288px;
    }
  }
  @media (max-width: 899px) {
    .editor {
      overflow-y: auto;
    }
  }
</style>
