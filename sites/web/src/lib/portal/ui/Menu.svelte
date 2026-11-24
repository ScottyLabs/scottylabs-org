<script lang="ts" module>
  import type { Attachment } from "svelte/attachments";

  /** Spread these onto the trigger button: `{#snippet trigger(props)}<button {...props}>` */
  export type MenuTriggerProps = {
    "aria-haspopup": "menu";
    "aria-expanded": "true" | "false";
    "aria-controls": string;
    onclick: (event: MouseEvent) => void;
    onkeydown: (event: KeyboardEvent) => void;
  } & Record<symbol, Attachment<HTMLElement>>;
</script>

<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import { createAttachmentKey } from "svelte/attachments";
  import type { MenuItem } from "./types.ts";

  let {
    items,
    label,
    open = $bindable(false),
    align = "end",
    width = 260,
    header,
    trigger,
    class: className,
  }: {
    /** "separator" draws a line between groups. */
    items: readonly (MenuItem | "separator")[];
    /** Names the menu for assistive technology. */
    label: string;
    open?: boolean;
    /** Which edge of the trigger the menu lines up with. */
    align?: "start" | "end";
    width?: number;
    /** Text above the items, such as who is signed in. Not focusable. */
    header?: Snippet;
    trigger: Snippet<[MenuTriggerProps]>;
    class?: string;
  } = $props();

  const uid = $props.id();
  const menuId = `menu-${uid}`;

  let root: HTMLElement | undefined = $state();
  let triggerElement: HTMLElement | null = null;
  const itemElements: (HTMLElement | undefined)[] = $state([]);

  const entries = $derived(
    items.map((item, index) => ({ item, index })).filter(
      (entry): entry is { item: MenuItem; index: number } => entry.item !== "separator",
    ),
  );
  const focusable = $derived(entries.filter((entry) => !entry.item.disabled));

  async function openMenu(focus: "first" | "last") {
    open = true;
    await tick();
    const target = focus === "first" ? focusable[0] : focusable.at(-1);
    if (target !== undefined) itemElements[target.index]?.focus();
  }

  function closeMenu(returnFocus: boolean) {
    open = false;
    if (returnFocus) triggerElement?.focus();
  }

  function onTriggerClick() {
    if (open) closeMenu(false);
    else void openMenu("first");
  }

  function onTriggerKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      void openMenu(event.key === "ArrowDown" ? "first" : "last");
    }
  }

  const attachmentKey = createAttachmentKey();
  const triggerAttachment: Attachment<HTMLElement> = (node) => {
    triggerElement = node;
    return () => {
      if (triggerElement === node) triggerElement = null;
    };
  };

  const triggerProps = $derived<MenuTriggerProps>({
    "aria-haspopup": "menu",
    "aria-expanded": open ? "true" : "false",
    "aria-controls": menuId,
    onclick: onTriggerClick,
    onkeydown: onTriggerKeydown,
    [attachmentKey]: triggerAttachment,
  });

  function moveFocus(from: number, direction: 1 | -1) {
    const position = focusable.findIndex((entry) => entry.index === from);
    const next = focusable[(position + direction + focusable.length) % focusable.length];
    if (next !== undefined) itemElements[next.index]?.focus();
  }

  function onMenuKeydown(event: KeyboardEvent) {
    const current = itemElements.findIndex((element) => element === document.activeElement);
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        moveFocus(current, 1);
        return;
      case "ArrowUp":
        event.preventDefault();
        moveFocus(current, -1);
        return;
      case "Home":
        event.preventDefault();
        if (focusable[0]) itemElements[focusable[0].index]?.focus();
        return;
      case "End": {
        event.preventDefault();
        const last = focusable.at(-1);
        if (last) itemElements[last.index]?.focus();
        return;
      }
      case "Escape":
        event.preventDefault();
        closeMenu(true);
        return;
      case "Tab":
        closeMenu(false);
        return;
      case " ":
        // Space activates links too, as it does buttons.
        if (document.activeElement instanceof HTMLAnchorElement) {
          event.preventDefault();
          document.activeElement.click();
        }
        return;
      default:
        typeahead(event, current);
    }
  }

  function typeahead(event: KeyboardEvent, current: number) {
    if (event.key.length !== 1 || event.ctrlKey || event.metaKey || event.altKey) return;
    const letter = event.key.toLowerCase();
    const position = focusable.findIndex((entry) => entry.index === current);
    const ordered = [...focusable.slice(position + 1), ...focusable.slice(0, position + 1)];
    const match = ordered.find((entry) => entry.item.label.toLowerCase().startsWith(letter));
    if (match !== undefined) itemElements[match.index]?.focus();
  }

  function activate(item: MenuItem) {
    closeMenu(item.href === undefined);
    item.onSelect?.();
  }

  function onDocumentPointerDown(event: PointerEvent) {
    if (event.target instanceof Node && root?.contains(event.target)) return;
    closeMenu(false);
  }

  $effect(() => {
    if (!open) return;
    document.addEventListener("pointerdown", onDocumentPointerDown, true);
    return () => {
      document.removeEventListener("pointerdown", onDocumentPointerDown, true);
    };
  });
</script>

<div class={["p-menu", className]} bind:this={root}>
  {@render trigger(triggerProps)}
  {#if open}
    <div class={["p-menu__popover", `p-menu__popover--${align}`]} style:width="{width}px">
      {#if header}
        <div class="p-menu__header">{@render header()}</div>
        <div class="p-menu__separator" aria-hidden="true"></div>
      {/if}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div id={menuId} class="p-menu__items" role="menu" aria-label={label} onkeydown={onMenuKeydown}>
        {#each items as item, index (index)}
          {#if item === "separator"}
            <div class="p-menu__separator" role="separator"></div>
          {:else if item.href !== undefined && !item.disabled}
            <a
              bind:this={itemElements[index]}
              class={["p-menu__item", item.danger && "p-menu__item--danger"]}
              role="menuitem"
              tabindex="-1"
              href={item.href}
              target={item.external ? "_blank" : undefined}
              rel={item.external ? "noopener noreferrer" : undefined}
              onclick={() => activate(item)}
            >
              {#if item.icon}<item.icon size={16} strokeWidth={1.75} aria-hidden="true" />{/if}
              <span>{item.label}</span>
              {#if item.external}<span class="p-sr-only">(opens in a new tab)</span>{/if}
            </a>
          {:else}
            <button
              bind:this={itemElements[index]}
              type="button"
              class={["p-menu__item", item.danger && "p-menu__item--danger"]}
              role="menuitem"
              tabindex="-1"
              disabled={item.disabled}
              aria-disabled={item.disabled ? "true" : undefined}
              onclick={() => activate(item)}
            >
              {#if item.icon}<item.icon size={16} strokeWidth={1.75} aria-hidden="true" />{/if}
              <span>{item.label}</span>
            </button>
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .p-menu {
    position: relative;
    display: inline-flex;
  }
  .p-menu__popover {
    position: absolute;
    top: calc(100% + 8px);
    z-index: 50;
    box-sizing: border-box;
    max-width: calc(100vw - 24px);
    padding: 8px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-pop);
    font-family: var(--p-font);
    animation: menu-in var(--p-duration) var(--p-ease);
  }
  .p-menu__popover--end {
    right: 0;
  }
  .p-menu__popover--start {
    left: 0;
  }
  .p-menu__header {
    padding: 8px 10px 10px;
  }
  .p-menu__items {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .p-menu__separator {
    height: 1px;
    margin: 2px 0;
    background: var(--p-border);
  }
  .p-menu__item {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 38px;
    padding: 9px 10px;
    border-radius: var(--p-radius-sm);
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
    text-align: left;
    text-decoration: none;
    cursor: pointer;

    &:hover,
    &:focus-visible {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: -2px;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: not-allowed;
      background: transparent;
    }
  }
  .p-menu__item--danger {
    color: var(--p-danger-text);
  }
  @keyframes menu-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .p-menu__popover {
      animation: none;
    }
  }
</style>
