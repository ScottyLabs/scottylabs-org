<script lang="ts">
  // Restricted rich text editing: bold, italic, links and lists, nothing else.
  //
  //   <Field label="Description">
  //     <RichTextEditor bind:value={doc.description} maxChars={4000} />
  //   </Field>
  //
  // `variant="inline"` is borderless for editing text in place on the committee page canvas, with
  // the dark format bar floating above the selection. Every change is normalized with
  // normalizeRichText(), so `value` only ever holds markup the server keeps.
  import "./rich-text.css";
  import { onMount, untrack } from "svelte";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Lock from "@lucide/svelte/icons/lock";
  import { getFieldContext } from "../portal/ui/field-context.ts";
  import FormatToolbar from "./FormatToolbar.svelte";
  import LinkPopover from "./LinkPopover.svelte";
  import {
    BLOCKED_INPUT_TYPES,
    type FormatCommand,
    type FormatState,
    NO_FORMATS,
    closestIn,
    collapseToEnd,
    createLink,
    ensureParagraph,
    insertHtml,
    insertLineBreak,
    insertableHtml,
    isVisuallyEmpty,
    rangeAtPoint,
    readFormats,
    removeLink,
    runFormat,
    selectContents,
    selectRange,
    selectionBox,
    selectionIn,
    useParagraphs,
  } from "./commands.ts";
  import { editorInvalid, isOverLimit, limitMessage } from "./limit.ts";
  import { linkText } from "./links.ts";
  import { type Box, floatingPosition } from "./position.ts";
  import {
    escapeAttribute,
    escapeHtml,
    normalizeRichText,
    plainTextLength,
    sanitize,
    textToHtml,
  } from "./sanitize.ts";

  let {
    value = $bindable(""),
    onchange,
    placeholder = "",
    maxChars,
    disabled = false,
    variant = "field",
    label,
    invalid,
    id: idProp,
    "aria-describedby": describedByProp,
    class: className,
  }: {
    /** Sanitized HTML. Empty content is "". */
    value?: string;
    /** Called with normalized HTML after every change. */
    onchange?: (html: string) => void;
    placeholder?: string;
    /** Shows a count of visible characters; past it the text is marked invalid with a message. */
    maxChars?: number;
    disabled?: boolean;
    /** "field" sits in a portal Field; "inline" edits text in place. */
    variant?: "field" | "inline";
    /** The accessible name when the editor is not inside a Field. */
    label?: string;
    /** Marks the text invalid. Text over `maxChars` is marked either way. */
    invalid?: boolean;
    id?: string;
    "aria-describedby"?: string;
    class?: string;
  } = $props();

  const field = getFieldContext();
  const uid = $props.id();
  const editorId = $derived(idProp ?? field?.id ?? `rte-${uid}`);
  const counterId = `rte-${uid}-counter`;
  const hintId = `rte-${uid}-hint`;
  const limitId = `rte-${uid}-limit`;
  const linkPopoverId = `rte-${uid}-link`;

  let wrapper: HTMLDivElement | undefined = $state();
  let root: HTMLDivElement | undefined = $state();
  let fieldToolbar: FormatToolbar | undefined = $state();
  let floatingToolbar: FormatToolbar | undefined = $state();

  let empty = $state(true);
  let length = $state(0);
  let formats = $state<FormatState>(NO_FORMATS);
  let focused = $state(false);
  let toolbarFocused = $state(false);
  let hasSelection = $state(false);
  let link = $state<{ initial: string; editing: boolean } | null>(null);
  let modifier = $state<"Cmd" | "Ctrl">("Ctrl");

  let anchor = $state<Box | null>(null);
  let barWidth = $state(0);
  let barHeight = $state(0);
  let viewportWidth = $state(0);
  let viewportHeight = $state(0);

  // The value this editor last showed or sent, so outside changes can be told from our own.
  let shown: string | null = null;
  let savedRange: Range | null = null;
  let draggingInside = false;

  const over = $derived(isOverLimit(length, maxChars));
  const overMessage = $derived(over && maxChars !== undefined ? limitMessage(maxChars) : "");
  const isInvalid = $derived(editorInvalid({ invalid, over, fieldInvalid: field?.invalid }));
  const describedBy = $derived(
    [
      describedByProp ?? field?.describedBy,
      maxChars === undefined ? undefined : counterId,
      over ? limitId : undefined,
      variant === "inline" ? hintId : undefined,
    ]
      .filter((part) => part !== undefined && part !== "")
      .join(" ") || undefined,
  );
  const showFloating = $derived(
    variant === "inline" &&
      !disabled &&
      focused &&
      (hasSelection || toolbarFocused || link !== null),
  );
  const floating = $derived(
    anchor !== null && barWidth > 0
      ? floatingPosition(
          anchor,
          { width: barWidth, height: barHeight },
          { width: viewportWidth, height: viewportHeight },
        )
      : null,
  );
  // The link popover opens under the selection, leaving the bar where it is.
  const popoverPlace = $derived(
    anchor === null
      ? null
      : {
          top: Math.round(Math.min(anchor.top + anchor.height + 8, viewportHeight - 180)),
          left: Math.round(Math.max(8, Math.min(anchor.left, viewportWidth - 348))),
        },
  );

  // Show `value` whenever it changes from outside, such as undo in the page editor.
  $effect(() => {
    const next = value;
    const element = root;
    if (element === undefined) return;
    untrack(() => {
      if (next === shown) return;
      shown = next;
      // Safe: sanitize() keeps only the allowed tags and checked links.
      element.innerHTML = sanitize(next);
      refresh(element, next);
    });
  });

  // Clicking the Field's label focuses the text, as it would an input.
  $effect(() => {
    if (field === undefined) return;
    const labelElement = document.querySelector(`#${CSS.escape(field.labelId)}`);
    if (labelElement === null) return;
    labelElement.addEventListener("click", focusEditor);
    return () => labelElement.removeEventListener("click", focusEditor);
  });

  onMount(() => {
    modifier = /Mac|iPhone|iPad/u.test(navigator.userAgent) ? "Cmd" : "Ctrl";
    document.addEventListener("selectionchange", trackSelection);
    return () => document.removeEventListener("selectionchange", trackSelection);
  });

  // While the floating bar shows, keep it next to the selection as the page scrolls.
  $effect(() => {
    if (!showFloating) return;
    window.addEventListener("scroll", reposition, { capture: true, passive: true });
    window.addEventListener("resize", reposition, { passive: true });
    return () => {
      window.removeEventListener("scroll", reposition, { capture: true });
      window.removeEventListener("resize", reposition);
    };
  });

  function reposition(): void {
    if (savedRange !== null) anchor = boxOf(savedRange);
  }

  function boxOf(range: Range): Box | null {
    const rect = selectionBox(range);
    if (rect === null) return null;
    return { top: rect.top, left: rect.left, width: rect.width, height: rect.height };
  }

  function refresh(element: HTMLElement, html: string): void {
    empty = isVisuallyEmpty(element);
    length = plainTextLength(html);
  }

  /** Reads the edited content, normalizes it and reports it when it changed. */
  function sync(): void {
    if (root === undefined) return;
    const html = normalizeRichText(root.innerHTML);
    refresh(root, html);
    if (html === shown) return;
    shown = html;
    value = html;
    onchange?.(html);
  }

  function trackSelection(): void {
    if (root === undefined) return;
    const range = selectionIn(root);
    if (range === null) {
      if (document.activeElement === root) hasSelection = false;
      return;
    }
    savedRange = range.cloneRange();
    hasSelection = !range.collapsed;
    formats = readFormats(root);
    if (variant === "inline") anchor = boxOf(range);
  }

  /** Puts focus back in the text with the selection it had. */
  function focusEditor(): void {
    if (root === undefined || disabled) return;
    // A toolbar button keeps the selection; a popover input takes it, so it is put back.
    const current = selectionIn(root)?.cloneRange() ?? null;
    if (document.activeElement === root && current !== null) return;
    root.focus({ preventScroll: true });
    const wanted =
      current ??
      (savedRange !== null && root.contains(savedRange.commonAncestorContainer) ? savedRange : null);
    if (wanted !== null) selectRange(wanted);
  }

  function format(command: FormatCommand): void {
    if (root === undefined || disabled) return;
    focusEditor();
    runFormat(command, root);
    sync();
    formats = readFormats(root);
  }

  function currentLink(): HTMLElement | null {
    if (root === undefined) return null;
    const range = selectionIn(root) ?? savedRange;
    return range === null ? null : closestIn(range.startContainer, root, ["a"]);
  }

  function openLink(): void {
    if (root === undefined || disabled) return;
    const range = selectionIn(root);
    if (range !== null) savedRange = range.cloneRange();
    const existing = currentLink();
    link = { initial: existing?.getAttribute("href") ?? "", editing: existing !== null };
  }

  function closeLink(): void {
    link = null;
    focusEditor();
  }

  function saveLink(href: string): void {
    link = null;
    focusEditor();
    const existing = currentLink();
    const range = root === undefined ? null : selectionIn(root);
    if (existing !== null) {
      selectContents(existing);
      createLink(href);
    } else if (range === null || range.collapsed) {
      insertHtml(`<a href="${escapeAttribute(href)}">${escapeHtml(linkText(href))}</a>`);
    } else {
      createLink(href);
    }
    collapseToEnd();
    sync();
  }

  function unlink(): void {
    link = null;
    focusEditor();
    const existing = currentLink();
    if (existing !== null) selectContents(existing);
    removeLink();
    collapseToEnd();
    sync();
  }

  function focusToolbar(): void {
    if (variant === "field") {
      fieldToolbar?.focus();
      return;
    }
    if (savedRange !== null) anchor = boxOf(savedRange);
    toolbarFocused = true;
    // The floating bar renders on the next frame.
    requestAnimationFrame(() => floatingToolbar?.focus());
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "Enter" && event.shiftKey && !event.altKey && !event.metaKey && !event.ctrlKey) {
      if (insertLineBreak()) {
        event.preventDefault();
        sync();
      }
      return;
    }
    if (event.altKey && event.key === "F10") {
      event.preventDefault();
      focusToolbar();
      return;
    }
    const mod = modifier === "Cmd" ? event.metaKey : event.ctrlKey;
    if (!mod || event.altKey) return;
    const key = event.key.toLowerCase();
    let command: FormatCommand | "link" | "blocked" | null = null;
    if (event.shiftKey && event.code === "Digit7") command = "numbered";
    else if (event.shiftKey && event.code === "Digit8") command = "bulleted";
    else if (!event.shiftKey && key === "b") command = "bold";
    else if (!event.shiftKey && key === "i") command = "italic";
    else if (!event.shiftKey && key === "k") command = "link";
    else if (key === "u") command = "blocked";
    if (command === null) return;
    event.preventDefault();
    if (command === "link") openLink();
    else if (command !== "blocked") format(command);
  }

  function onbeforeinput(event: InputEvent): void {
    if (BLOCKED_INPUT_TYPES.has(event.inputType)) event.preventDefault();
  }

  function insertClean(html: string): void {
    if (html === "") return;
    insertHtml(insertableHtml(html));
    sync();
  }

  function onpaste(event: ClipboardEvent): void {
    const data = event.clipboardData;
    if (data === null) return;
    event.preventDefault();
    const html = data.getData("text/html");
    insertClean(
      html === "" ? normalizeRichText(textToHtml(data.getData("text/plain"))) : normalizeRichText(html),
    );
  }

  function ondrop(event: DragEvent): void {
    // Moving text within the editor is the browser's job; the result is normalized on input.
    if (draggingInside) return;
    const data = event.dataTransfer;
    event.preventDefault();
    if (data === null || data.files.length > 0) return;
    const range = rangeAtPoint(event.clientX, event.clientY);
    root?.focus({ preventScroll: true });
    if (range !== null) selectRange(range);
    const html = data.getData("text/html");
    insertClean(normalizeRichText(html === "" ? textToHtml(data.getData("text/plain")) : html));
  }

  function onfocus(): void {
    if (root === undefined || disabled) return;
    useParagraphs();
    ensureParagraph(root);
    empty = isVisuallyEmpty(root);
  }

  function onfocusin(event: FocusEvent): void {
    focused = true;
    const target = event.target;
    toolbarFocused =
      target instanceof Element && target.closest(".rte__float, .rte__bar") !== null;
  }

  function onfocusout(event: FocusEvent): void {
    const next = event.relatedTarget;
    if (next instanceof Node && wrapper?.contains(next)) return;
    focused = false;
    toolbarFocused = false;
    hasSelection = false;
    link = null;
    // Show exactly what was saved, so leftovers such as an empty paragraph disappear.
    if (root !== undefined && root.innerHTML !== (shown ?? "")) {
      // Safe: `shown` came from normalizeRichText() or sanitize().
      root.innerHTML = shown ?? "";
      empty = isVisuallyEmpty(root);
    }
  }
</script>

<svelte:window bind:innerWidth={viewportWidth} bind:innerHeight={viewportHeight} />

<div
  bind:this={wrapper}
  class={[
    "rte",
    `rte--${variant}`,
    isInvalid && "rte--invalid",
    disabled && "rte--disabled",
    className,
  ]}
  {onfocusin}
  {onfocusout}
>
  {#if variant === "field"}
    <div class="rte__bar">
      <FormatToolbar
        bind:this={fieldToolbar}
        {formats}
        controls={editorId}
        {disabled}
        {modifier}
        linkOpen={link !== null}
        {linkPopoverId}
        onformat={format}
        onlink={openLink}
        onescape={focusEditor}
      />
      {#if maxChars !== undefined}
        <span id={counterId} class={["rte__counter", over && "rte__counter--over"]}>
          <span aria-hidden="true">{length.toLocaleString("en-US")} / {maxChars.toLocaleString("en-US")}</span>
          <span class="p-sr-only">{length} of {maxChars} characters</span>
        </span>
      {/if}
    </div>
    {#if link !== null}
      <div class="rte__popover">
        <LinkPopover
          id={linkPopoverId}
          initial={link.initial}
          editing={link.editing}
          onsave={saveLink}
          onremove={unlink}
          oncancel={closeLink}
        />
      </div>
    {/if}
  {/if}

  <div class="rte__surface">
    <div
      bind:this={root}
      id={editorId}
      class="rte__text rich-text"
      contenteditable={disabled ? "false" : "true"}
      role="textbox"
      tabindex={disabled ? -1 : 0}
      aria-multiline="true"
      aria-labelledby={field?.labelId}
      aria-label={field === undefined ? label : undefined}
      aria-describedby={describedBy}
      aria-invalid={isInvalid ? "true" : undefined}
      aria-required={field?.required === true ? "true" : undefined}
      aria-disabled={disabled ? "true" : undefined}
      aria-placeholder={placeholder === "" ? undefined : placeholder}
      spellcheck="true"
      oninput={sync}
      {onkeydown}
      {onbeforeinput}
      {onpaste}
      {ondrop}
      {onfocus}
      ondragstart={() => (draggingInside = true)}
      ondragend={() => (draggingInside = false)}
    ></div>
    {#if empty && placeholder !== ""}
      <span class="rte__placeholder" aria-hidden="true">{placeholder}</span>
    {/if}
  </div>

  {#if variant === "inline"}
    <span id={hintId} class="p-sr-only">
      Select text to format it. Press Alt+F10 to reach the formatting buttons.
    </span>
    {#if maxChars !== undefined}
      <span id={counterId} class="p-sr-only">{length} of {maxChars} characters</span>
    {/if}
    {#if over && maxChars !== undefined}
      <!-- Sits under the text without moving the canvas around it. -->
      <p id={limitId} class="rte__limit rte__limit--inline">
        <span class="rte__limit-count" aria-hidden="true">
          {length.toLocaleString("en-US")} / {maxChars.toLocaleString("en-US")}
        </span>
        <span>{overMessage}</span>
      </p>
    {/if}
  {/if}
  <!-- Always present, so the message is announced when the text goes over the limit. -->
  <span class="p-sr-only" aria-live="polite">{overMessage}</span>

  {#if showFloating}
    <div
      class="rte__float"
      class:rte__float--placed={floating !== null}
      style:top={floating === null ? undefined : `${floating.top.toString()}px`}
      style:left={floating === null ? undefined : `${floating.left.toString()}px`}
      bind:offsetWidth={barWidth}
      bind:offsetHeight={barHeight}
    >
      <div class="rte__float-bar">
        <FormatToolbar
          bind:this={floatingToolbar}
          tone="dark"
          {formats}
          controls={editorId}
          {modifier}
          linkOpen={link !== null}
          {linkPopoverId}
          onformat={format}
          onlink={openLink}
          onescape={focusEditor}
        >
          {#snippet trailing()}
            <span class="rte__divider" aria-hidden="true"></span>
            <span class="rte__theme" title="Fonts, sizes and colors are set by the theme.">
              <Lock size={12} strokeWidth={2} aria-hidden="true" />
              Theme type
            </span>
          {/snippet}
        </FormatToolbar>
      </div>
    </div>
    {#if link !== null}
      <div
        class="rte__float-popover"
        style:top={popoverPlace === null ? undefined : `${popoverPlace.top.toString()}px`}
        style:left={popoverPlace === null ? undefined : `${popoverPlace.left.toString()}px`}
      >
        <LinkPopover
          id={linkPopoverId}
          initial={link.initial}
          editing={link.editing}
          onsave={saveLink}
          onremove={unlink}
          oncancel={closeLink}
        />
      </div>
    {/if}
  {/if}
</div>
{#if variant === "field" && over}
  <p id={limitId} class="rte__limit">
    <CircleAlert size={14} strokeWidth={2} aria-hidden="true" />
    <span>{overMessage}</span>
  </p>
{/if}

<style>
  .rte {
    position: relative;
    min-width: 0;
  }

  /* Field: a bordered box with the toolbar on top, like the portal's text area. */
  .rte--field {
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    transition: border-color var(--p-duration) var(--p-ease);

    &:hover:not(.rte--disabled) {
      border-color: #b8b8b8;
    }
    &:has(.rte__text:focus-visible) {
      border-color: var(--p-focus);
      outline: 2px solid var(--p-focus);
      outline-offset: -1px;
    }
  }
  .rte--field.rte--invalid {
    border-color: var(--p-danger);
  }
  .rte--field.rte--disabled {
    background: var(--p-surface-muted);
    color: var(--p-text-secondary);
  }
  .rte__bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 8px 4px 4px;
    border-bottom: 1px solid var(--p-border-muted);
  }
  .rte__counter {
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .rte__counter--over {
    color: var(--p-danger-text);
  }
  .rte__popover {
    position: absolute;
    top: 42px;
    left: 4px;
    z-index: 20;
  }

  .rte__surface {
    position: relative;
  }
  .rte__text {
    min-height: 1lh;
    outline: none;
    white-space: normal;
    cursor: text;
  }
  .rte--field .rte__text {
    min-height: 120px;
    max-height: 480px;
    overflow-y: auto;
    padding: 12px;
    font-size: var(--p-field-font);
    line-height: var(--p-field-line);
  }
  .rte--disabled .rte__text {
    cursor: not-allowed;
  }
  .rte__placeholder {
    position: absolute;
    top: 0;
    left: 0;
    color: var(--p-placeholder);
    pointer-events: none;
    user-select: none;
  }
  .rte--field .rte__placeholder {
    top: 12px;
    left: 12px;
    font-size: var(--p-field-font);
    line-height: var(--p-field-line);
  }

  /* Inline: the text takes the type of the block it sits in. */
  .rte--inline .rte__text {
    border-radius: 2px;
  }
  /* The committee canvas draws its own selection box around the block and can turn this off. */
  .rte--inline .rte__text:focus-visible {
    outline: var(--rte-inline-focus-outline, 2px solid var(--p-focus));
    outline-offset: 4px;
  }
  .rte--inline.rte--invalid .rte__text {
    outline: 2px solid var(--p-danger);
    outline-offset: 4px;
  }

  .rte__limit {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    color: var(--p-danger-text);
    font-family: var(--p-font);
    font-size: 0.8125rem;
    font-weight: 400;
    line-height: 18px;
    letter-spacing: 0;

    & :global(svg) {
      flex: none;
      margin-top: 2px;
    }
  }
  .rte__limit--inline {
    position: absolute;
    top: calc(100% + 10px);
    left: 0;
    z-index: 2;
    align-items: baseline;
    gap: 8px;
    max-width: min(100%, 420px);
    padding: 4px 8px;
    border-radius: var(--p-radius-sm);
    background: var(--p-danger-surface);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    font-size: 0.75rem;
    line-height: 16px;
    text-align: left;
    text-transform: none;
  }
  .rte__limit-count {
    flex: none;
    font-family: var(--p-mono);
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .rte__float-popover {
    position: fixed;
    z-index: 61;
  }
  .rte__float {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 60;
    display: flex;
    /* Measured before it is placed; still focusable so Alt+F10 can reach it straight away. */
    opacity: 0;
  }
  .rte__float--placed {
    opacity: 1;
    animation: rte-float-in 120ms var(--p-ease);
  }
  .rte__float-bar {
    display: flex;
    align-items: center;
    padding: 4px 6px;
    border-radius: 9px;
    background: var(--p-toast);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.14);
  }
  .rte__divider {
    width: 1px;
    height: 16px;
    margin: 0 1px;
    background: rgba(255, 255, 255, 0.25);
  }
  .rte__theme {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 6px;
    color: rgba(255, 255, 255, 0.78);
    font-family: var(--p-font);
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
    white-space: nowrap;
  }

  @keyframes rte-float-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .rte__float--placed {
      animation: none;
    }
  }
</style>
