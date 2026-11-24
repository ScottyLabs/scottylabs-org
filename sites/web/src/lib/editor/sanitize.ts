// Restricted rich text for committee page text blocks and project descriptions, with the same rules as services/site/src/richtext.rs: only p, br, strong, em, a, ul, ol and li survive; links must be https or mailto and open in a new tab; everything else is unwrapped to its text, and script and style are dropped with their content.
//
// In the browser, HTML is parsed with DOMParser, which never runs scripts or loads images. Where there is no DOM (tests and prerendering), ./html-parse.ts stands in. Either way the output is rebuilt from scratch: text is escaped and only checked links are written back.

import { ELEMENT_NODE, type HtmlNode, TEXT_NODE, parseHtmlFragment } from "./html-parse.ts";

export type AllowedTag = "p" | "br" | "strong" | "em" | "a" | "ul" | "ol" | "li";

const ALLOWED_TAGS: ReadonlySet<string> = new Set<AllowedTag>([
  "p",
  "br",
  "strong",
  "em",
  "a",
  "ul",
  "ol",
  "li",
]);

/** Dropped with everything inside, as the server does. */
const DROPPED_TAGS = new Set(["script", "style"]);

/** Attribute values every link gets, matching the server. */
export const LINK_TARGET = "_blank";
export const LINK_REL = "noopener noreferrer nofollow";

/** Deep enough for any real document; deeper content is left out rather than risk the stack. */
const MAX_DEPTH = 200;

type CleanNode =
  | { kind: "text"; text: string }
  | { kind: "element"; tag: AllowedTag; href: string | null; children: CleanNode[] };

function isAllowedTag(tag: string): tag is AllowedTag {
  return ALLOWED_TAGS.has(tag);
}

/** Parses HTML into nodes, with DOMParser when the platform has it. */
export function parseHtml(html: string): readonly HtmlNode[] {
  if (typeof DOMParser === "function") {
    // A body start tag keeps leading <style> or <title> in the body, where it is handled like anywhere else, instead of moving it to the head.
    const doc = new DOMParser().parseFromString(`<!doctype html><body>${html}`, "text/html");
    return Array.from(doc.body.childNodes);
  }
  return parseHtmlFragment(html);
}

function tagOf(node: HtmlNode): string {
  return node.nodeName.toLowerCase();
}

function attribute(node: HtmlNode, name: string): string | null {
  return node.getAttribute?.(name) ?? null;
}

/** URL parsing drops tabs and line breaks, so "java\nscript:" would pass; refuse them instead. */
function hasControlCharacter(text: string): boolean {
  for (const char of text) {
    if ((char.codePointAt(0) ?? 0) < 0x20) return true;
  }
  return false;
}

/**
 * The link as it will be written, or null when it is not allowed. Only absolute `https:` links
 * with a host and `mailto:` links pass; relative links are refused, as on the server.
 */
export function safeHref(href: string | null): string | null {
  if (href === null) return null;
  const trimmed = href.trim();
  if (trimmed === "" || hasControlCharacter(trimmed)) return null;
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }
  if (url.protocol === "https:" && url.hostname !== "") return url.href;
  if (url.protocol === "mailto:" && url.pathname !== "") return url.href;
  return null;
}

export function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\u{A0}", "&nbsp;");
}

export function escapeAttribute(value: string): string {
  return escapeHtml(value).replaceAll('"', "&quot;");
}

function serialize(nodes: readonly CleanNode[]): string {
  let html = "";
  for (const node of nodes) {
    if (node.kind === "text") {
      html += escapeHtml(node.text);
    } else if (node.tag === "br") {
      html += "<br>";
    } else if (node.tag === "a") {
      html += `<a href="${escapeAttribute(node.href ?? "")}" target="${LINK_TARGET}" rel="${LINK_REL}">${serialize(node.children)}</a>`;
    } else {
      html += `<${node.tag}>${serialize(node.children)}</${node.tag}>`;
    }
  }
  return html;
}

function pushText(out: CleanNode[], text: string): void {
  if (text === "") return;
  const last = out.at(-1);
  if (last?.kind === "text") last.text += text;
  else out.push({ kind: "text", text });
}

function pushAll(out: CleanNode[], nodes: readonly CleanNode[]): void {
  for (const node of nodes) {
    if (node.kind === "text") pushText(out, node.text);
    else out.push(node);
  }
}

function cleanNodes(nodes: ArrayLike<HtmlNode>, inLink: boolean, depth: number): CleanNode[] {
  const out: CleanNode[] = [];
  if (depth > MAX_DEPTH) return out;
  for (const node of Array.from(nodes)) {
    if (node.nodeType === TEXT_NODE) {
      pushText(out, node.nodeValue ?? "");
      continue;
    }
    if (node.nodeType !== ELEMENT_NODE) continue;
    const tag = tagOf(node);
    if (DROPPED_TAGS.has(tag)) continue;
    const href = tag === "a" && !inLink ? safeHref(attribute(node, "href")) : null;
    const isLink = href !== null;
    const children = cleanNodes(node.childNodes, inLink || isLink, depth + 1);
    if (!isAllowedTag(tag) || (tag === "a" && !isLink)) {
      pushAll(out, children);
    } else if (tag === "br") {
      out.push({ kind: "element", tag, href: null, children: [] });
    } else {
      out.push({ kind: "element", tag, href, children });
    }
  }
  return out;
}

/**
 * Keeps only the allowed tags and links, the way the server does. Use it before `{@html}` on any
 * rich text, even text the server already cleaned.
 */
export function sanitize(html: string): string {
  if (html === "") return "";
  return serialize(cleanNodes(parseHtml(html), false, 0));
}

// ---------------------------------------------------------------------------
// Normalizing editor output and pasted content

/** Tags whose start and end break a paragraph. */
const BLOCK_TAGS = new Set([
  "address",
  "article",
  "aside",
  "blockquote",
  "caption",
  "center",
  "dd",
  "details",
  "dialog",
  "div",
  "dl",
  "dt",
  "fieldset",
  "figcaption",
  "figure",
  "footer",
  "form",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "header",
  "hgroup",
  "hr",
  "legend",
  "main",
  "nav",
  "p",
  "pre",
  "section",
  "summary",
  "table",
  "tbody",
  "td",
  "tfoot",
  "th",
  "thead",
  "tr",
]);

/** Content that is not prose: dropped from pasted and edited content. */
const IGNORED_TAGS = new Set([
  "audio",
  "button",
  "canvas",
  "embed",
  "head",
  "iframe",
  "img",
  "input",
  "link",
  "math",
  "meta",
  "noscript",
  "object",
  "picture",
  "script",
  "select",
  "style",
  "svg",
  "template",
  "textarea",
  "title",
  "video",
]);

const HEADING_TAGS = new Set(["h1", "h2", "h3", "h4", "h5", "h6", "th"]);

interface Marks {
  readonly strong: boolean;
  readonly em: boolean;
  readonly href: string | null;
}

type Inline = { kind: "text"; text: string; marks: Marks } | { kind: "br" };

interface ListItem {
  blocks: Block[];
}

interface ListBlock {
  kind: "ul" | "ol";
  items: ListItem[];
}

type Block = { kind: "p"; inline: Inline[] } | ListBlock;

const NO_MARKS: Marks = { strong: false, em: false, href: null };

/** Collects inline content into paragraphs, starting a new one at each block boundary. */
class Flow {
  readonly #blocks: Block[] = [];
  #inline: Inline[] = [];

  inline(item: Inline): void {
    this.#inline.push(item);
  }

  block(block: Block): void {
    this.breakParagraph();
    this.#blocks.push(block);
  }

  breakParagraph(): void {
    const inline = tidyInline(this.#inline);
    if (inline.length > 0) this.#blocks.push({ kind: "p", inline });
    this.#inline = [];
  }

  finish(): Block[] {
    this.breakParagraph();
    return this.#blocks;
  }
}

/** Trims spaces at the edges of lines and drops line breaks at the start and end. */
function tidyInline(items: readonly Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const item of items) {
    const previous = out.at(-1);
    if (item.kind === "br") {
      if (previous?.kind === "text") previous.text = previous.text.trimEnd();
      out.push(item);
    } else {
      // Collapsed spaces at the start of a line or after another space would not show.
      const afterSpace =
        previous === undefined || previous.kind === "br" || /\s$/u.test(previous.text);
      const text = afterSpace ? item.text.trimStart() : item.text;
      if (text !== "") out.push({ ...item, text });
    }
  }
  const last = out.at(-1);
  if (last?.kind === "text") last.text = last.text.trimEnd();
  while (out[0]?.kind === "br") out.shift();
  while (out.at(-1)?.kind === "br") out.pop();
  const visible = out.some((item) => item.kind === "text" && item.text.trim() !== "");
  return visible ? out.filter((item) => item.kind === "br" || item.text !== "") : [];
}

function styleValue(style: string, property: string): string | null {
  const match = new RegExp(`(?:^|;)\\s*${property}\\s*:\\s*([^;]+)`, "iu").exec(style);
  return match?.[1]?.trim().toLowerCase() ?? null;
}

function marksFor(node: HtmlNode, tag: string, marks: Marks): Marks {
  let { strong, em, href } = marks;
  if (tag === "strong" || tag === "b" || HEADING_TAGS.has(tag)) strong = true;
  if (tag === "em" || tag === "i") em = true;
  if (tag === "a") href = safeHref(attribute(node, "href")) ?? href;
  const style = attribute(node, "style");
  if (style !== null) {
    // Word processors mark formatting with styles, and Google Docs wraps whole documents in
    // <b style="font-weight:normal">, so styles win over the tag.
    const weight = styleValue(style, "font-weight");
    if (weight !== null && /^(bold|bolder|[6-9]00)/u.test(weight)) strong = true;
    if (weight !== null && /^(normal|lighter|[1-5]00)/u.test(weight)) strong = false;
    const fontStyle = styleValue(style, "font-style");
    if (fontStyle !== null && /^(italic|oblique)/u.test(fontStyle)) em = true;
    if (fontStyle === "normal") em = false;
  }
  return { strong, em, href };
}

function readListItem(node: HtmlNode, marks: Marks, depth: number): ListItem {
  const flow = new Flow();
  walk(node.childNodes, marks, flow, depth + 1);
  return { blocks: flow.finish() };
}

function readList(node: HtmlNode, kind: "ul" | "ol", marks: Marks, depth: number): ListBlock {
  const items: ListItem[] = [];
  let loose: Flow | null = null;
  const flushLoose = () => {
    const blocks = loose?.finish() ?? [];
    if (blocks.length > 0) items.push({ blocks });
    loose = null;
  };
  for (const child of Array.from(node.childNodes)) {
    const tag = child.nodeType === ELEMENT_NODE ? tagOf(child) : "";
    if (tag === "li") {
      flushLoose();
      items.push(readListItem(child, marks, depth));
    } else if ((tag === "ul" || tag === "ol") && items.length > 0 && loose === null) {
      // A list placed straight inside another list belongs to the item before it.
      items.at(-1)?.blocks.push(readList(child, tag, marks, depth + 1));
    } else {
      loose ??= new Flow();
      walk([child], marks, loose, depth + 1);
    }
  }
  flushLoose();
  return { kind, items: items.filter((item) => item.blocks.length > 0) };
}

function walk(nodes: ArrayLike<HtmlNode>, marks: Marks, flow: Flow, depth: number): void {
  if (depth > MAX_DEPTH) return;
  for (const node of Array.from(nodes)) {
    if (node.nodeType === TEXT_NODE) {
      // Browsers keep typed spaces with no-break spaces; a lone one between words is a space.
      const text = (node.nodeValue ?? "")
        .replaceAll(/[ \t\n\r\f]+/gu, " ")
        .replaceAll(/(?<!\s)\u{A0}(?!\s)/gu, " ");
      if (text !== "") flow.inline({ kind: "text", text, marks });
      continue;
    }
    if (node.nodeType !== ELEMENT_NODE) continue;
    const tag = tagOf(node);
    if (IGNORED_TAGS.has(tag)) continue;
    if (tag === "br") {
      flow.inline({ kind: "br" });
    } else if (tag === "ul" || tag === "ol") {
      const list = readList(node, tag, marks, depth);
      if (list.items.length > 0) flow.block(list);
    } else if (tag === "li") {
      const item = readListItem(node, marks, depth);
      if (item.blocks.length > 0) flow.block({ kind: "ul", items: [item] });
    } else if (BLOCK_TAGS.has(tag)) {
      flow.breakParagraph();
      walk(node.childNodes, marksFor(node, tag, marks), flow, depth + 1);
      flow.breakParagraph();
    } else {
      walk(node.childNodes, marksFor(node, tag, marks), flow, depth + 1);
    }
  }
}

/** Opens and closes tags so each run of text sits inside exactly its marks. */
function serializeInline(items: readonly Inline[]): string {
  let html = "";
  let open: string[] = [];
  let openKeys: string[] = [];
  const closeFrom = (index: number) => {
    for (let i = open.length - 1; i >= index; i -= 1) html += `</${open[i] ?? ""}>`;
    open = open.slice(0, index);
    openKeys = openKeys.slice(0, index);
  };
  for (const item of items) {
    if (item.kind === "br") {
      html += "<br>";
      continue;
    }
    const wanted: { tag: string; key: string; html: string }[] = [];
    if (item.marks.href !== null) {
      wanted.push({
        tag: "a",
        key: `a ${item.marks.href}`,
        html: `<a href="${escapeAttribute(item.marks.href)}" target="${LINK_TARGET}" rel="${LINK_REL}">`,
      });
    }
    if (item.marks.strong) wanted.push({ tag: "strong", key: "strong", html: "<strong>" });
    if (item.marks.em) wanted.push({ tag: "em", key: "em", html: "<em>" });
    let same = 0;
    while (same < openKeys.length && openKeys[same] === wanted[same]?.key) same += 1;
    closeFrom(same);
    for (const mark of wanted.slice(same)) {
      html += mark.html;
      open.push(mark.tag);
      openKeys.push(mark.key);
    }
    html += escapeHtml(item.text);
  }
  closeFrom(0);
  return html;
}

function serializeBlocks(blocks: readonly Block[]): string {
  return blocks
    .map((block) => {
      if (block.kind === "p") return `<p>${serializeInline(block.inline)}</p>`;
      const items = block.items.map((item) => `<li>${serializeItem(item.blocks)}</li>`).join("");
      return `<${block.kind}>${items}</${block.kind}>`;
    })
    .join("");
}

/** Paragraphs inside a list item become lines of the item; nested lists stay lists. */
function serializeItem(blocks: readonly Block[]): string {
  let html = "";
  let previous: Block | undefined;
  for (const block of blocks) {
    if (block.kind === "p") {
      if (previous?.kind === "p") html += "<br>";
      html += serializeInline(block.inline);
    } else {
      html += serializeBlocks([block]);
    }
    previous = block;
  }
  return html;
}

/**
 * Turns edited or pasted HTML into tidy restricted rich text: bold and italic from any markup or
 * style become strong and em, headings become bold paragraphs, other blocks become paragraphs,
 * loose text is wrapped in paragraphs and empty paragraphs are dropped. The result passes
 * `sanitize` unchanged, and is "" when there is no text.
 */
export function normalizeRichText(html: string): string {
  if (html === "") return "";
  const flow = new Flow();
  walk(parseHtml(html), NO_MARKS, flow, 0);
  const out = serializeBlocks(flow.finish());
  return plainText(out) === "" ? "" : out;
}

/** Plain text pasted into the editor: blank lines separate paragraphs, other line breaks stay. */
export function textToHtml(text: string): string {
  return text
    .replaceAll(/\r\n?/gu, "\n")
    .split(/\n\s*\n/u)
    .map((paragraph) => paragraph.trim())
    .filter((paragraph) => paragraph !== "")
    .map((paragraph) => {
      const lines = paragraph.split("\n").map((line) => escapeHtml(line));
      return `<p>${lines.join("<br>")}</p>`;
    })
    .join("");
}

// ---------------------------------------------------------------------------
// Plain text

/** Elements followed by a space in plain text, so words from separate lines stay apart. */
const TEXT_BOUNDARIES = new Set(["p", "li", "br", "ul", "ol"]);

function collectText(nodes: ArrayLike<HtmlNode>, parts: string[], depth: number): void {
  if (depth > MAX_DEPTH) return;
  for (const node of Array.from(nodes)) {
    if (node.nodeType === TEXT_NODE) {
      parts.push(node.nodeValue ?? "");
    } else if (node.nodeType === ELEMENT_NODE) {
      const tag = tagOf(node);
      if (DROPPED_TAGS.has(tag)) continue;
      collectText(node.childNodes, parts, depth + 1);
      if (TEXT_BOUNDARIES.has(tag)) parts.push(" ");
    }
  }
}

/** The visible text of rich text, with tags removed and whitespace collapsed, as on the server. */
export function plainText(html: string): string {
  if (html === "") return "";
  const parts: string[] = [];
  collectText(parseHtml(html), parts, 0);
  return parts.join("").replaceAll(/\s+/gu, " ").trim();
}

/** Characters of visible text, counted the way the server counts them (code points). */
export function plainTextLength(html: string): number {
  // Array.from splits by code point, like Rust's chars().
  return Array.from(plainText(html)).length;
}
