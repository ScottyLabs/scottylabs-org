<script lang="ts">
  // The formatting buttons of RichTextEditor, as one toolbar tab stop: arrow keys, Home and End
  // move between buttons, and Escape goes back to the text.
  import type { Snippet } from "svelte";
  import Bold from "@lucide/svelte/icons/bold";
  import Italic from "@lucide/svelte/icons/italic";
  import LinkIcon from "@lucide/svelte/icons/link";
  import List from "@lucide/svelte/icons/list";
  import ListOrdered from "@lucide/svelte/icons/list-ordered";
  import type { IconComponent } from "../portal/ui/types.ts";
  import type { FormatCommand, FormatState } from "./commands.ts";

  let {
    formats,
    controls,
    tone = "light",
    disabled = false,
    linkOpen = false,
    linkPopoverId,
    modifier = "Ctrl",
    onformat,
    onlink,
    onescape,
    trailing,
  }: {
    formats: FormatState;
    /** The id of the text box the buttons change. */
    controls: string;
    /** "dark" is the floating bar on the committee page canvas. */
    tone?: "light" | "dark";
    disabled?: boolean;
    linkOpen?: boolean;
    linkPopoverId?: string;
    /** "Cmd" on Apple devices, shown in the tooltips. */
    modifier?: "Cmd" | "Ctrl";
    onformat: (command: FormatCommand) => void;
    onlink: () => void;
    onescape: () => void;
    /** Shown after the buttons, such as a character count. */
    trailing?: Snippet;
  } = $props();

  type ItemId = FormatCommand | "link";
  interface Item {
    id: ItemId;
    label: string;
    icon: IconComponent;
    key?: string;
  }

  const ITEMS: readonly Item[] = [
    { id: "bold", label: "Bold", icon: Bold, key: "B" },
    { id: "italic", label: "Italic", icon: Italic, key: "I" },
    { id: "link", label: "Link", icon: LinkIcon, key: "K" },
    { id: "bulleted", label: "Bulleted list", icon: List },
    { id: "numbered", label: "Numbered list", icon: ListOrdered },
  ];

  let active = $state(0);
  const buttons: (HTMLButtonElement | undefined)[] = $state([]);

  /** Moves focus to the toolbar, for Alt+F10 in the text. */
  export function focus(): void {
    buttons[active]?.focus();
  }

  function pressed(id: ItemId): boolean | undefined {
    return id === "link" ? undefined : formats[id];
  }

  function title(item: Item): string {
    if (item.key === undefined) return item.label;
    return `${item.label} (${modifier}+${item.key})`;
  }

  function shortcut(item: Item): string | undefined {
    if (item.key === undefined) return undefined;
    return `${modifier === "Cmd" ? "Meta" : "Control"}+${item.key}`;
  }

  function activate(item: Item): void {
    if (item.id === "link") onlink();
    else onformat(item.id);
  }

  function move(to: number): void {
    active = (to + ITEMS.length) % ITEMS.length;
    buttons[active]?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowRight":
        event.preventDefault();
        move(active + 1);
        break;
      case "ArrowLeft":
        event.preventDefault();
        move(active - 1);
        break;
      case "Home":
        event.preventDefault();
        move(0);
        break;
      case "End":
        event.preventDefault();
        move(ITEMS.length - 1);
        break;
      case "Escape":
        event.preventDefault();
        onescape();
        break;
      default:
    }
  }
</script>

<div
  class={["rte-toolbar", `rte-toolbar--${tone}`]}
  role="toolbar"
  aria-label="Formatting"
  aria-controls={controls}
>
  {#each ITEMS as item, index (item.id)}
    {@const Icon = item.icon}
    <button
      bind:this={buttons[index]}
      type="button"
      class="rte-toolbar__button"
      class:rte-toolbar__button--on={pressed(item.id) === true ||
        (item.id === "link" && (formats.link || linkOpen))}
      tabindex={index === active ? 0 : -1}
      aria-label={item.label}
      aria-pressed={pressed(item.id)}
      aria-haspopup={item.id === "link" ? "dialog" : undefined}
      aria-expanded={item.id === "link" ? linkOpen : undefined}
      aria-controls={item.id === "link" && linkOpen ? linkPopoverId : undefined}
      aria-keyshortcuts={shortcut(item)}
      title={title(item)}
      {disabled}
      onmousedown={(event) => event.preventDefault()}
      onfocus={() => (active = index)}
      onclick={() => activate(item)}
      {onkeydown}
    >
      <Icon size={tone === "dark" ? 14 : 16} strokeWidth={2} aria-hidden="true" />
    </button>
  {/each}
  {@render trailing?.()}
</div>

<style>
  .rte-toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    font-family: var(--p-font);
  }
  .rte-toolbar__button {
    box-sizing: border-box;
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:disabled {
      cursor: not-allowed;
      opacity: 0.45;
    }
  }

  .rte-toolbar--light .rte-toolbar__button {
    width: 30px;
    height: 30px;
    color: var(--p-text);

    &:hover:not(:disabled) {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .rte-toolbar--light .rte-toolbar__button--on {
    background: var(--p-surface-strong);

    &:hover:not(:disabled) {
      background: var(--p-surface-strong);
    }
  }

  .rte-toolbar--dark .rte-toolbar__button {
    width: 26px;
    height: 26px;
    color: #ffffff;

    &:hover:not(:disabled) {
      background: rgba(255, 255, 255, 0.1);
    }
    &:focus-visible {
      outline: 2px solid #ffffff;
      outline-offset: 0;
    }
  }
  .rte-toolbar--dark .rte-toolbar__button--on {
    background: rgba(255, 255, 255, 0.16);
  }
</style>
