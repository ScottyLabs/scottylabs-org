<script lang="ts">
  // One line of plain text that the page editor can change in place. On public pages it is just
  // the element with its text; empty text renders nothing there. In the editor an empty field
  // shows its placeholder, and while `editable` the element itself takes typing.
  //   <EditableText as="h2" class="cb-heading" value={props.heading} {editable} label="Heading" onchange={...} />
  import type { Attachment } from "svelte/attachments";
  import { oneLine } from "./limits.ts";

  let {
    value,
    as = "p",
    editor = false,
    editable = false,
    label,
    placeholder = "",
    maxLength,
    onchange,
    class: className,
  }: {
    value: string;
    as?: "p" | "h2" | "h3" | "h4" | "span" | "div" | "blockquote" | "figcaption";
    /** In the page editor, where empty fields show their placeholder. */
    editor?: boolean;
    /** Takes typing. Only for the selected block. */
    editable?: boolean;
    /** Names the field for assistive technology, such as "Heading". */
    label: string;
    placeholder?: string;
    maxLength?: number;
    onchange?: (value: string) => void;
    class?: string;
  } = $props();

  // Shows the value without touching the text while it already matches, so the caret stays put
  // while someone types. Outside changes such as undo replace the text.
  const showValue: Attachment<HTMLElement> = (element) => {
    const text = value;
    if (element.textContent === text) return;
    element.textContent = text;
    if (document.activeElement === element) placeCaretAtEnd(element);
  };

  function placeCaretAtEnd(element: HTMLElement) {
    const selection = window.getSelection();
    if (selection === null) return;
    const range = document.createRange();
    range.selectNodeContents(element);
    range.collapse(false);
    selection.removeAllRanges();
    selection.addRange(range);
  }

  function oninput(event: Event) {
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) return;
    onchange?.(oneLine(element.textContent ?? ""));
  }

  function onbeforeinput(event: InputEvent) {
    if (event.inputType === "insertParagraph" || event.inputType === "insertLineBreak") {
      event.preventDefault();
      return;
    }
    if (maxLength === undefined || !event.inputType.startsWith("insert")) return;
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) return;
    const selected = window.getSelection()?.toString().length ?? 0;
    const adding = event.data?.length ?? event.dataTransfer?.getData("text/plain").length ?? 0;
    if ((element.textContent ?? "").length - selected + adding > maxLength) event.preventDefault();
  }

  function onkeydown(event: KeyboardEvent) {
    // Enter and Escape finish editing; the canvas keeps the block selected.
    if (event.key === "Enter" || event.key === "Escape") {
      event.preventDefault();
      if (event.currentTarget instanceof HTMLElement) event.currentTarget.blur();
    }
  }
</script>

{#if editable}
  <svelte:element
    this={as}
    class={[className, "cb-editable", value === "" && "cb-editable--empty"]}
    contenteditable="plaintext-only"
    role="textbox"
    tabindex="0"
    aria-label={label}
    aria-multiline="false"
    data-placeholder={placeholder}
    spellcheck="true"
    {@attach showValue}
    {oninput}
    {onbeforeinput}
    {onkeydown}
  ></svelte:element>
{:else if value !== ""}
  <svelte:element this={as} class={className}>{value}</svelte:element>
{:else if editor && placeholder !== ""}
  <svelte:element this={as} class={[className, "cb-placeholder"]}>{placeholder}</svelte:element>
{/if}

<style>
  .cb-editable {
    border-radius: 4px;
    outline: 1px dashed transparent;
    outline-offset: 3px;
    cursor: text;
    transition: outline-color var(--p-duration) var(--p-ease);

    &:hover {
      outline-color: rgba(36, 164, 255, 0.6);
    }
    &:focus {
      outline: 2px solid var(--p-focus);
    }
  }
  .cb-editable--empty::before {
    color: var(--p-placeholder);
    content: attr(data-placeholder);
    pointer-events: none;
  }
</style>
