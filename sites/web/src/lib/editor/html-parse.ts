// A small HTML fragment parser for places without DOMParser: Deno tests and prerendering.
// In the browser, sanitize.ts parses with DOMParser instead. This parser only has to keep the right text and formatting. It is never trusted for safety: sanitize.ts rebuilds its output from scratch, escapes every piece of text and keeps only the allowed tags and checked links.

export const ELEMENT_NODE = 1;
export const TEXT_NODE = 3;

/** The parts of a DOM node the sanitizer reads. Real DOM nodes have the same shape. */
export interface HtmlNode {
  readonly nodeType: number;
  /** Upper case for HTML elements, as in the DOM. */
  readonly nodeName: string;
  readonly nodeValue: string | null;
  readonly childNodes: ArrayLike<HtmlNode>;
  getAttribute?(name: string): string | null;
}

interface ParsedElement extends HtmlNode {
  readonly tag: string;
  readonly childNodes: HtmlNode[];
}

function elementNode(tag: string, attributes: ReadonlyMap<string, string>): ParsedElement {
  return {
    nodeType: ELEMENT_NODE,
    nodeName: tag.toUpperCase(),
    nodeValue: null,
    tag,
    childNodes: [],
    getAttribute: (name) => attributes.get(name.toLowerCase()) ?? null,
  };
}

function textNode(text: string): HtmlNode {
  return { nodeType: TEXT_NODE, nodeName: "#text", nodeValue: text, childNodes: [] };
}

const VOID_TAGS = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

/** Elements whose content is text up to their end tag, never markup. */
const RAW_TEXT_TAGS = new Set([
  "script",
  "style",
  "textarea",
  "title",
  "xmp",
  "iframe",
  "noembed",
  "noframes",
  "noscript",
]);

/** Raw text elements whose text is shown, so entities in it are decoded. */
const DECODED_RAW_TEXT_TAGS = new Set(["textarea", "title"]);

/** Opening one of these closes an open paragraph, as browsers do. */
const CLOSES_PARAGRAPH = new Set([
  "address",
  "article",
  "aside",
  "blockquote",
  "dd",
  "details",
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
  "li",
  "main",
  "menu",
  "nav",
  "ol",
  "p",
  "pre",
  "section",
  "table",
  "ul",
]);

const NAMED_ENTITIES: Readonly<Record<string, string>> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: "\u{A0}",
  shy: "\u{AD}",
  copy: "\u{A9}",
  reg: "\u{AE}",
  trade: "\u{2122}",
  deg: "\u{B0}",
  times: "\u{D7}",
  middot: "\u{B7}",
  bull: "\u{2022}",
  hellip: "\u{2026}",
  ndash: "\u{2013}",
  mdash: "\u{2014}",
  lsquo: "\u{2018}",
  rsquo: "\u{2019}",
  ldquo: "\u{201C}",
  rdquo: "\u{201D}",
  laquo: "\u{AB}",
  raquo: "\u{BB}",
  eacute: "\u{E9}",
  egrave: "\u{E8}",
  aacute: "\u{E1}",
  agrave: "\u{E0}",
  ouml: "\u{F6}",
  uuml: "\u{FC}",
  auml: "\u{E4}",
  ccedil: "\u{E7}",
  ntilde: "\u{F1}",
};

const ENTITY = /&(?:#[xX]([0-9a-fA-F]{1,6});?|#([0-9]{1,7});?|([a-zA-Z][a-zA-Z0-9]{1,31});)/gu;

function codePointText(value: number): string {
  const valid = value > 0 && value <= 0x10ffff && (value < 0xd800 || value > 0xdfff);
  return String.fromCodePoint(valid ? value : 0xfffd);
}

/** Decodes numeric references and the common named ones. Unknown names stay as written. */
export function decodeEntities(text: string): string {
  if (!text.includes("&")) return text;
  return text.replaceAll(
    ENTITY,
    (match, hex: string | undefined, decimal: string | undefined, name: string | undefined) => {
      if (hex !== undefined) return codePointText(Number.parseInt(hex, 16));
      if (decimal !== undefined) return codePointText(Number(decimal));
      return (name === undefined ? undefined : NAMED_ENTITIES[name]) ?? match;
    },
  );
}

function isWhitespace(char: string | undefined): boolean {
  return char === " " || char === "\n" || char === "\t" || char === "\r" || char === "\f";
}

function isLetter(char: string | undefined): boolean {
  return char !== undefined && /^[a-zA-Z]$/u.test(char);
}

interface Tag {
  name: string;
  attributes: Map<string, string>;
  /** Index just past the closing `>`. */
  end: number;
}

/** Reads a start tag at `start` (which holds `<`). Null when the tag never closes. */
function readStartTag(html: string, start: number): Tag | null {
  let i = start + 1;
  let name = "";
  while (i < html.length && !isWhitespace(html[i]) && html[i] !== "/" && html[i] !== ">") {
    name += html[i];
    i += 1;
  }
  const attributes = new Map<string, string>();
  while (i < html.length) {
    while (isWhitespace(html[i]) || html[i] === "/") i += 1;
    if (i >= html.length) return null;
    if (html[i] === ">") return { name: name.toLowerCase(), attributes, end: i + 1 };
    let attribute = "";
    while (
      i < html.length &&
      !isWhitespace(html[i]) &&
      html[i] !== "=" &&
      html[i] !== ">" &&
      html[i] !== "/"
    ) {
      attribute += html[i];
      i += 1;
    }
    while (isWhitespace(html[i])) i += 1;
    let value = "";
    if (html[i] === "=") {
      i += 1;
      while (isWhitespace(html[i])) i += 1;
      const quote = html[i];
      if (quote === '"' || quote === "'") {
        const close = html.indexOf(quote, i + 1);
        if (close === -1) return null;
        value = html.slice(i + 1, close);
        i = close + 1;
      } else {
        while (i < html.length && !isWhitespace(html[i]) && html[i] !== ">") {
          value += html[i];
          i += 1;
        }
      }
    }
    const key = attribute.toLowerCase();
    // The first of repeated attributes wins, as in browsers.
    if (key !== "" && !attributes.has(key)) attributes.set(key, decodeEntities(value));
  }
  return null;
}

class TreeBuilder {
  readonly root = elementNode("#root", new Map());
  readonly #stack: ParsedElement[] = [this.root];

  get current(): ParsedElement {
    return this.#stack.at(-1) ?? this.root;
  }

  text(raw: string, decode = true): void {
    if (raw === "") return;
    const text = decode ? decodeEntities(raw) : raw;
    const siblings = this.current.childNodes;
    const last = siblings.at(-1);
    if (last?.nodeType === TEXT_NODE) {
      siblings[siblings.length - 1] = textNode(`${last.nodeValue ?? ""}${text}`);
    } else {
      siblings.push(textNode(text));
    }
  }

  open(tag: Tag): ParsedElement {
    if (CLOSES_PARAGRAPH.has(tag.name)) this.#closeOpenParagraph();
    if (tag.name === "li") this.#closeOpenListItem();
    // Links cannot nest: a new link ends the open one, as in browsers.
    if (tag.name === "a") this.close("a");
    const element = elementNode(tag.name, tag.attributes);
    this.current.childNodes.push(element);
    if (!VOID_TAGS.has(tag.name)) this.#stack.push(element);
    return element;
  }

  close(name: string): void {
    const index = this.#stack.findLastIndex((element) => element.tag === name);
    // Index 0 is the root. A stray end tag is ignored, as browsers do.
    if (index > 0) this.#stack.length = index;
  }

  #closeOpenParagraph(): void {
    for (let i = this.#stack.length - 1; i > 0; i -= 1) {
      const tag = this.#stack[i]?.tag;
      if (tag === "p") {
        this.#stack.length = i;
        return;
      }
      if (tag === "button" || tag === "td" || tag === "th" || tag === "li") return;
    }
  }

  #closeOpenListItem(): void {
    for (let i = this.#stack.length - 1; i > 0; i -= 1) {
      const tag = this.#stack[i]?.tag;
      if (tag === "li") {
        this.#stack.length = i;
        return;
      }
      if (tag === "ul" || tag === "ol") return;
    }
  }
}

/** Skips a comment, doctype or processing instruction at `start`. Returns where text resumes. */
function skipDeclaration(html: string, start: number): number {
  const isComment = html.startsWith("<!--", start);
  const closing = isComment ? "-->" : ">";
  const end = html.indexOf(closing, start + (isComment ? 4 : 1));
  return end === -1 ? html.length : end + closing.length;
}

function readEndTag(html: string, start: number): { name: string; end: number } {
  let i = start + 2;
  let name = "";
  while (i < html.length && !isWhitespace(html[i]) && html[i] !== ">") {
    name += html[i];
    i += 1;
  }
  const close = html.indexOf(">", i);
  return { name: name.toLowerCase(), end: close === -1 ? html.length : close + 1 };
}

/** Reads the text of a raw text element and returns where its end tag finishes. */
function readRawText(
  html: string,
  lower: string,
  start: number,
  tag: string,
): { text: string; end: number } {
  const close = lower.indexOf(`</${tag}`, start);
  if (close === -1) return { text: html.slice(start), end: html.length };
  const end = html.indexOf(">", close);
  return { text: html.slice(start, close), end: end === -1 ? html.length : end + 1 };
}

/** Parses an HTML fragment into nodes shaped like the DOM's. */
export function parseHtmlFragment(html: string): HtmlNode[] {
  const builder = new TreeBuilder();
  const lower = html.toLowerCase();
  let i = 0;
  while (i < html.length) {
    const lt = html.indexOf("<", i);
    if (lt === -1) {
      builder.text(html.slice(i));
      break;
    }
    builder.text(html.slice(i, lt));
    const next = html[lt + 1];
    if (next === "!" || next === "?") {
      i = skipDeclaration(html, lt);
    } else if (next === "/" && isLetter(html[lt + 2])) {
      const tag = readEndTag(html, lt);
      builder.close(tag.name);
      i = tag.end;
    } else if (isLetter(next)) {
      const tag = readStartTag(html, lt);
      if (tag === null) break;
      const element = builder.open(tag);
      i = tag.end;
      if (RAW_TEXT_TAGS.has(tag.name)) {
        const raw = readRawText(html, lower, i, tag.name);
        element.childNodes.push(
          textNode(DECODED_RAW_TEXT_TAGS.has(tag.name) ? decodeEntities(raw.text) : raw.text),
        );
        builder.close(tag.name);
        i = raw.end;
      }
    } else {
      builder.text("<", false);
      i = lt + 1;
    }
  }
  return builder.root.childNodes;
}
