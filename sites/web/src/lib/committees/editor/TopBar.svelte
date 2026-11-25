<script lang="ts">
  // The editor's own top bar (Figma 109:7058): back to the list, the page and its path, Draft or
  // Published, whether it is saved, the grid, undo and redo, desktop or phone width, Preview and
  // Publish.
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import Monitor from "@lucide/svelte/icons/monitor";
  import Redo2 from "@lucide/svelte/icons/redo-2";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Smartphone from "@lucide/svelte/icons/smartphone";
  import Undo2 from "@lucide/svelte/icons/undo-2";
  import { Badge, Button, DogMark, IconButton, Menu, type MenuItem } from "../../portal/ui/index.ts";
  import { committeePath } from "../links.ts";
  import { editorBadge, savedLabel } from "../status.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    device = $bindable("desktop"),
    showGrid = $bindable(false),
    onpreview,
    onpublish,
    ondiscard,
  }: {
    editor: PageEditor;
    device?: "desktop" | "phone";
    showGrid?: boolean;
    onpreview: () => void;
    onpublish: () => void;
    ondiscard: () => void;
  } = $props();

  // "Saved 2 min ago" moves on by itself.
  let now = $state(new Date());
  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 30_000);
    return () => clearInterval(timer);
  });

  const saveText = $derived.by(() => {
    switch (editor.saveState) {
      case "saved":
        return editor.savedAt === null ? "All changes saved" : savedLabel(editor.savedAt, now);
      case "unsaved":
      case "saving":
        return "Saving";
      case "offline":
        return "Not saved yet. Trying again";
      case "invalid":
        return "Not saved. Fix the marked block";
      case "conflict":
        return "Not saved";
    }
  });

  const menuItems = $derived<MenuItem[]>([
    {
      label: "View published page",
      icon: ExternalLink,
      href: committeePath(editor.slug),
      external: true,
      disabled: editor.published === null,
    },
    {
      label: "Discard unpublished changes",
      icon: RotateCcw,
      danger: true,
      disabled: !editor.writable || editor.status !== "unpublished_changes",
      onSelect: ondiscard,
    },
  ]);
</script>

<header class="bar">
  <div class="bar__start">
    <IconButton href="/portal/committees/" label="Back to committee pages" icon={ArrowLeft} />
    <DogMark width={30} class="bar__mark" />
    <div class="bar__title">
      <h1>{editor.committee.name} committee page</h1>
      <p class="bar__path">/committees/{editor.slug}</p>
    </div>
    {#if editor.writable}
      <Badge tone={editorBadge(editor.status) === "Draft" ? "neutral" : "success"}>{editorBadge(editor.status)}</Badge>
      <p class="bar__saved" role="status" aria-live="polite">{saveText}</p>
    {:else}
      <Badge tone="warning">Read only</Badge>
    {/if}
  </div>

  <div class="bar__device" role="group" aria-label="Canvas width">
    <button
      type="button"
      class="bar__device-button"
      aria-pressed={device === "desktop"}
      onclick={() => (device = "desktop")}
    >
      <Monitor size={18} strokeWidth={1.75} aria-hidden="true" /><span class="p-sr-only">Desktop</span>
    </button>
    <button
      type="button"
      class="bar__device-button"
      aria-pressed={device === "phone"}
      onclick={() => (device = "phone")}
    >
      <Smartphone size={18} strokeWidth={1.75} aria-hidden="true" /><span class="p-sr-only">Phone</span>
    </button>
  </div>

  <div class="bar__end">
    <IconButton
      label={showGrid ? "Hide the grid" : "Show the grid"}
      icon={LayoutGrid}
      aria-pressed={showGrid}
      class={showGrid ? "bar__pressed" : undefined}
      onclick={() => (showGrid = !showGrid)}
    />
    {#if editor.writable}
      <IconButton label="Undo" icon={Undo2} disabled={!editor.canUndo} onclick={() => editor.undo()} aria-keyshortcuts="Control+Z Meta+Z" />
      <IconButton
        label="Redo"
        icon={Redo2}
        disabled={!editor.canRedo}
        onclick={() => editor.redo()}
        aria-keyshortcuts="Control+Shift+Z Meta+Shift+Z Control+Y"
      />
    {/if}
    <span class="bar__divider" aria-hidden="true"></span>
    <Button variant="outline" onclick={onpreview}>Preview</Button>
    {#if editor.writable}
      <Button onclick={onpublish}>Publish</Button>
    {/if}
    <Menu label="More page actions" items={menuItems} width={280}>
      {#snippet trigger(props)}
        <button {...props} type="button" class="bar__more" aria-label="More page actions">
          <Ellipsis size={18} strokeWidth={1.75} aria-hidden="true" />
        </button>
      {/snippet}
    </Menu>
  </div>
</header>

<style>
  .bar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 16px;
    box-sizing: border-box;
    height: 56px;
    padding: 0 16px 0 12px;
    border-bottom: 1px solid var(--p-border);
    background: var(--p-surface);
  }
  .bar__start,
  .bar__end {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .bar__end {
    justify-content: flex-end;
    gap: 6px;
  }
  .bar :global(.bar__mark) {
    flex: none;
    color: var(--p-text);
  }
  .bar__title {
    min-width: 0;
    margin-right: 6px;

    & h1 {
      overflow: hidden;
      margin: 0;
      color: var(--p-text);
      font-size: 1rem;
      font-weight: 500;
      line-height: 22px;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
  }
  .bar__path {
    margin: 0;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .bar__saved {
    margin: 0;
    overflow: hidden;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar__device {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--p-radius);
    background: var(--p-surface-strong);
  }
  .bar__device-button {
    display: grid;
    place-items: center;
    width: 40px;
    height: 30px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text-secondary);
    cursor: pointer;

    &[aria-pressed="true"] {
      background: var(--p-surface);
      color: var(--p-text);
      box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }
  .bar :global(.bar__pressed) {
    background: var(--p-info-surface);
    color: var(--p-info-text);
  }
  .bar__divider {
    width: 1px;
    height: 24px;
    margin: 0 4px;
    background: var(--p-border);
  }
  .bar__more {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--p-radius);
    color: var(--p-text);
    cursor: pointer;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
    }
  }

  @media (max-width: 1180px) {
    .bar :global(.bar__mark),
    .bar__path {
      display: none;
    }
  }
</style>
