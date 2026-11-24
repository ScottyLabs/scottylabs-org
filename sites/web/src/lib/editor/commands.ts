// Editing commands and selection helpers for RichTextEditor.
//
// Formatting goes through document.execCommand. It is deprecated, but it is still the only way to change a contenteditable that keeps the browser's own undo history, and every browser supports these commands. Whatever markup the browser produces is normalized afterwards.

export type FormatCommand = "bold" | "italic" | "bulleted" | "numbered";

export interface FormatState {
  bold: boolean;
  italic: boolean;
  bulleted: boolean;
  numbered: boolean;
  /** The caret or selection starts inside a link. */
  link: boolean;
}

export const NO_FORMATS: FormatState = {
  bold: false,
  italic: false,
  bulleted: false,
  numbered: false,
  link: false,
};

const COMMANDS: Record<FormatCommand, string> = {
  bold: "bold",
  italic: "italic",
  bulleted: "insertUnorderedList",
  numbered: "insertOrderedList",
};

/** Input the editor never accepts, such as underline from Cmd+U or text alignment. */
export const BLOCKED_INPUT_TYPES: ReadonlySet<string> = new Set([
  "formatUnderline",
  "formatStrikeThrough",
  "formatSuperscript",
  "formatSubscript",
  "formatJustifyFull",
  "formatJustifyCenter",
  "formatJustifyRight",
  "formatJustifyLeft",
  "formatIndent",
  "formatOutdent",
  "formatSetBlockTextDirection",
  "formatSetInlineTextDirection",
  "formatBackColor",
  "formatFontColor",
  "formatFontName",
  "insertHorizontalRule",
]);

function exec(command: string, value?: string): void {
  // oxlint-disable-next-line typescript/no-deprecated
  document.execCommand(command, false, value);
}

function queryState(command: string): boolean {
  try {
    // oxlint-disable-next-line typescript/no-deprecated
    return document.queryCommandState(command);
  } catch {
    return false;
  }
}

export function runFormat(command: FormatCommand, root: HTMLElement): void {
  if (command === "bold" || command === "italic") {
    exec(COMMANDS[command]);
    return;
  }
  // Chrome turns a <p> into <p><ul>...</ul></p>, which is not valid HTML, leaves a line taken out of a list without a paragraph, and drops the caret at the start of the line. Fix all three, putting the caret back by its text position, which the command does not change.
  const before = textSelection(root);
  exec(COMMANDS[command]);
  for (const paragraph of root.querySelectorAll(":scope > p:has(> ul, > ol)")) {
    paragraph.replaceWith(...Array.from(paragraph.childNodes));
  }
  wrapLooseLines(root);
  if (before !== null) selectTextRange(root, before);
}

const BLOCK_ELEMENTS = new Set(["P", "UL", "OL", "DIV"]);

/** Wraps text and inline elements sitting directly in `root` in paragraphs. */
function wrapLooseLines(root: HTMLElement): void {
  let run: ChildNode[] = [];
  const wrap = (before: ChildNode | null) => {
    const visible = run.some((node) => node.nodeName === "BR" || (node.textContent ?? "") !== "");
    if (visible) {
      const paragraph = document.createElement("p");
      root.insertBefore(paragraph, before);
      paragraph.append(...run);
    }
    run = [];
  };
  for (const child of Array.from(root.childNodes)) {
    if (BLOCK_ELEMENTS.has(child.nodeName)) wrap(child);
    else run.push(child);
  }
  wrap(null);
}

interface TextSelection {
  start: number;
  end: number;
  /** The caret sat at the start of a line, so a position on a boundary means the next line. */
  atLineStart: boolean;
  collapsed: boolean;
}

/** The selection as character positions in the text of `root`. */
function textSelection(root: HTMLElement): TextSelection | null {
  const range = selectionIn(root);
  // On an empty line there is no text to measure from; the browser's own caret is right there.
  if (range === null || !(range.startContainer instanceof Text)) return null;
  const lead = document.createRange();
  lead.selectNodeContents(root);
  lead.setEnd(range.startContainer, range.startOffset);
  const start = lead.toString().length;
  return {
    start,
    end: start + range.toString().length,
    atLineStart: range.startOffset === 0,
    collapsed: range.collapsed,
  };
}

/** Selects the text between two character positions in `root`. */
function selectTextRange(root: HTMLElement, positions: TextSelection): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  let seen = 0;
  let startSet = false;
  for (let node = walker.nextNode(); node instanceof Text; node = walker.nextNode()) {
    const length = node.data.length;
    const startHere = positions.atLineStart
      ? positions.start < seen + length || (positions.start === seen && length === 0)
      : positions.start <= seen + length;
    if (!startSet && startHere) {
      range.setStart(node, positions.start - seen);
      startSet = true;
      if (positions.collapsed) {
        range.collapse(true);
        selectRange(range);
        return;
      }
    }
    if (startSet && positions.end <= seen + length) {
      range.setEnd(node, positions.end - seen);
      selectRange(range);
      return;
    }
    seen += length;
  }
}

/**
 * A line break within the paragraph, for Shift+Enter. Returns false where the browser has no
 * such command (Firefox), which already inserts a <br> for Shift+Enter by itself.
 */
export function insertLineBreak(): boolean {
  // oxlint-disable-next-line typescript/no-deprecated
  if (!document.queryCommandSupported("insertLineBreak")) return false;
  exec("insertLineBreak");
  return true;
}

export function insertHtml(html: string): void {
  exec("insertHTML", html);
}

export function createLink(href: string): void {
  exec("createLink", href);
}

export function removeLink(): void {
  exec("unlink");
}

/** Makes Enter start a <p> rather than a <div>. */
export function useParagraphs(): void {
  exec("defaultParagraphSeparator", "p");
  exec("styleWithCSS", "false");
}

/** The selection, when it lies inside `root`. */
export function selectionIn(root: HTMLElement): Range | null {
  const selection = document.getSelection();
  if (selection === null || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  return root.contains(range.commonAncestorContainer) ? range : null;
}

export function selectRange(range: Range): void {
  const selection = document.getSelection();
  if (selection === null) return;
  selection.removeAllRanges();
  selection.addRange(range);
}

export function selectContents(node: Node): void {
  const range = document.createRange();
  range.selectNodeContents(node);
  selectRange(range);
}

export function collapseToEnd(): void {
  document.getSelection()?.collapseToEnd();
}

/** The nearest element with one of `tags` from `node` up to, but not including, `root`. */
export function closestIn(
  node: Node | null,
  root: HTMLElement,
  tags: readonly string[],
): HTMLElement | null {
  let current: Node | null = node;
  while (current !== null && current !== root) {
    if (current instanceof HTMLElement && tags.includes(current.tagName.toLowerCase())) {
      return current;
    }
    current = current.parentNode;
  }
  return null;
}

export function readFormats(root: HTMLElement): FormatState {
  const range = selectionIn(root);
  if (range === null) return NO_FORMATS;
  const list = closestIn(range.startContainer, root, ["ul", "ol"]);
  return {
    bold: queryState("bold"),
    italic: queryState("italic"),
    bulleted: list?.tagName === "UL",
    numbered: list?.tagName === "OL",
    link: closestIn(range.startContainer, root, ["a"]) !== null,
  };
}

/** A box for the selection, falling back to its element when the range has no size. */
export function selectionBox(range: Range): DOMRect | null {
  const rect = range.getBoundingClientRect();
  if (rect.width > 0 || rect.height > 0) return rect;
  const node = range.startContainer;
  const element = node instanceof Element ? node : node.parentElement;
  return element?.getBoundingClientRect() ?? null;
}

/** The caret position under a point, for dropping content where the pointer is. */
export function rangeAtPoint(x: number, y: number): Range | null {
  if (typeof document.caretPositionFromPoint === "function") {
    const position = document.caretPositionFromPoint(x, y);
    if (position === null) return null;
    const range = document.createRange();
    range.setStart(position.offsetNode, position.offset);
    range.collapse(true);
    return range;
  }
  // Safari before 26 has only the older, non-standard method.
  if (!("caretRangeFromPoint" in document)) return null;
  // oxlint-disable-next-line typescript/no-deprecated
  return document.caretRangeFromPoint(x, y);
}

/** True when the editor shows nothing, not even an empty list item. */
export function isVisuallyEmpty(root: HTMLElement): boolean {
  return (root.textContent ?? "").trim() === "" && root.querySelector("li") === null;
}

/** Gives an empty editor a paragraph to type into, so the first line is a <p> too. */
export function ensureParagraph(root: HTMLElement): void {
  if (root.childNodes.length > 0 && !isVisuallyEmpty(root)) return;
  root.innerHTML = "<p><br></p>";
  const paragraph = root.firstChild;
  if (paragraph === null) return;
  const range = document.createRange();
  range.setStart(paragraph, 0);
  range.collapse(true);
  selectRange(range);
}

const SINGLE_PARAGRAPH = /^<p>((?:(?!<\/?p>).)*)<\/p>$/su;

/**
 * Normalized HTML ready to insert at the caret. A single paragraph goes in as its inline content,
 * so pasting a phrase continues the current paragraph instead of starting a new one.
 */
export function insertableHtml(html: string): string {
  return SINGLE_PARAGRAPH.exec(html)?.[1] ?? html;
}
