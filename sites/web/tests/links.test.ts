// Every link that opens a new tab says rel="noopener noreferrer", so the page it opens can neither reach back into this one through window.opener nor learn which page sent it. This reads the markup of every component, so a new link without it fails here. Run from sites/web with `deno test -A tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access, typescript/no-unsafe-argument, typescript/strict-boolean-expressions */

import assert from "node:assert/strict";
import { LINK_REL, LINK_TARGET } from "../src/lib/editor/sanitize.ts";

/** Components that add rel="noopener noreferrer" themselves whenever they get target="_blank". */
const ADDS_REL = new Set(["Button", "IconButton"]);

interface Tag {
  name: string;
  text: string;
  line: number;
}

/** The web app's source, wherever the tests are run from. */
const SOURCE = decodeURIComponent(new URL("../src", import.meta.url).pathname);

async function* svelteFiles(dir: string): AsyncGenerator<string> {
  for await (const entry of Deno.readDir(dir)) {
    const path = `${dir}/${entry.name}`;
    if (entry.isDirectory) yield* svelteFiles(path);
    else if (entry.name.endsWith(".svelte") || entry.name.endsWith(".html")) yield path;
  }
}

/** The markup only: scripts, styles and comments become spaces, so line numbers stay right. */
function markupOf(source: string): string {
  return source.replaceAll(
    /<script[\s>][\s\S]*?<\/script>|<style[\s>][\s\S]*?<\/style>|<!--[\s\S]*?-->/gu,
    (block) => block.replaceAll(/[^\n]/gu, " "),
  );
}

/** The index just past a quoted string or `{expression}` that starts at `open`. */
function skipValue(text: string, open: number): number {
  const first = text[open];
  if (first === '"' || first === "'" || first === "`") {
    const close = text.indexOf(first, open + 1);
    return close === -1 ? text.length : close + 1;
  }
  let depth = 0;
  for (let index = open; index < text.length; index += 1) {
    const char = text[index];
    if (char === '"' || char === "'" || char === "`") {
      index = skipValue(text, index) - 1;
    } else if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) return index + 1;
    }
  }
  return text.length;
}

/** Each start tag with its attributes, reading past `>` inside quotes and `{expressions}`. */
function startTags(markup: string): Tag[] {
  const tags: Tag[] = [];
  for (const match of markup.matchAll(/<([A-Za-z][\w.:-]*)/gu)) {
    const start = match.index;
    let index = start + match[0].length;
    while (index < markup.length && markup[index] !== ">") {
      const char = markup[index] ?? "";
      index = char === '"' || char === "'" || char === "{" ? skipValue(markup, index) : index + 1;
    }
    const line = markup.slice(0, start).split("\n").length;
    tags.push({ name: match[1] ?? "", text: markup.slice(start, index + 1), line });
  }
  return tags;
}

/**
 * A start tag's attributes, from name to the source of the value (quoted or an `{expression}`),
 * or true for a bare attribute. A `{name}` shorthand counts as `name={name}`; spreads are skipped.
 */
function attributesOf(tag: string): Map<string, string | true> {
  const found = new Map<string, string | true>();
  const end = tag.endsWith(">") ? tag.length - 1 : tag.length;
  let index = /^<[\w.:-]+/u.exec(tag)?.[0].length ?? 1;
  while (index < end) {
    const char = tag[index] ?? "";
    if (/[\s/]/u.test(char)) {
      index += 1;
    } else if (char === "{") {
      const close = skipValue(tag, index);
      const inner = tag.slice(index + 1, close - 1).trim();
      if (/^[\w$]+$/u.test(inner)) found.set(inner, true);
      index = close;
    } else {
      const name = /^[^\s=/>{"']+/u.exec(tag.slice(index))?.[0] ?? char;
      index += name.length;
      while (/\s/u.test(tag[index] ?? "")) index += 1;
      if (tag[index] !== "=") {
        found.set(name, true);
        continue;
      }
      index += 1;
      while (/\s/u.test(tag[index] ?? "")) index += 1;
      const open = tag[index] ?? "";
      const close =
        open === '"' || open === "'" || open === "{"
          ? skipValue(tag, index)
          : index + (/^[^\s>]*/u.exec(tag.slice(index))?.[0].length ?? 0);
      found.set(name, tag.slice(index, close));
      index = close;
    }
  }
  return found;
}

/** What is wrong with one start tag, or null. */
function problem(tag: Tag): string | null {
  const attributes = attributesOf(tag.text);
  const target = attributes.get("target") ?? null;
  if (target === null || ADDS_REL.has(tag.name)) return null;
  const blank = target !== true && target.includes("_blank");
  // A target passed through from a prop, which may be "_blank": the rel beside it decides.
  const passedThrough = target === true || (target.startsWith("{") && !blank);
  if (!blank && !passedThrough) return null;
  const rel = attributes.get("rel") ?? null;
  if (rel === null) return "can open a new tab but has no rel";
  if (passedThrough) return null;
  if (rel === true || !rel.includes("noopener") || !rel.includes("noreferrer")) {
    return `opens a new tab with rel=${String(rel)} instead of "noopener noreferrer"`;
  }
  return null;
}

Deno.test('every link that opens a new tab has rel="noopener noreferrer"', async () => {
  const problems: string[] = [];
  let checked = 0;
  for await (const file of svelteFiles(SOURCE)) {
    const markup = markupOf(await Deno.readTextFile(file));
    for (const tag of startTags(markup)) {
      if (attributesOf(tag.text).has("target")) checked += 1;
      const found = problem(tag);
      if (found !== null)
        problems.push(`src${file.slice(SOURCE.length)}:${tag.line} <${tag.name}> ${found}`);
    }
  }
  assert.deepEqual(problems, []);
  // The scan found the links it is meant to check.
  assert.ok(checked > 20, `only ${checked} links with a target were found`);
});

Deno.test("rich text links open safely too", () => {
  assert.equal(LINK_TARGET, "_blank");
  assert.ok(LINK_REL.split(" ").includes("noopener"));
  assert.ok(LINK_REL.split(" ").includes("noreferrer"));
});

/** The problems the scan finds in `markup`. */
function check(markup: string): string[] {
  return startTags(markupOf(markup))
    .map((tag) => problem(tag))
    .filter((found) => found !== null);
}

Deno.test("the scan catches new-tab links without the right rel", () => {
  assert.equal(check('<a href="https://x.org" target="_blank">x</a>').length, 1);
  assert.equal(check('<a href={url} target="_blank" rel="noreferrer">x</a>').length, 1);
  assert.equal(check('<a href={url}\n  target="_blank"\n  rel="noopener">x</a>').length, 1);
  assert.equal(check('<a target={external ? "_blank" : undefined} href={url}>x</a>').length, 1);
  assert.equal(check('<svelte:element this="a" target="_blank">x</svelte:element>').length, 1);
  assert.equal(check('<Link href="/" target="_blank">x</Link>').length, 1);
  // These are fine.
  assert.equal(check('<a href={url} target="_blank" rel="noopener noreferrer">x</a>').length, 0);
  assert.equal(
    check('<a href={url} target="_blank" rel="noopener noreferrer nofollow">x</a>').length,
    0,
  );
  assert.equal(
    check(
      '<a target={a ? "_blank" : undefined} rel={a ? "noopener noreferrer" : undefined} onclick={() => a > 1}>x</a>',
    ).length,
    0,
  );
  assert.equal(check('<Button href="/" target="_blank">Site</Button>').length, 0);
  assert.equal(
    check('<a {href} {target} rel={target === "_blank" ? "noopener noreferrer" : undefined}>x</a>')
      .length,
    0,
  );
  assert.equal(check('<a href="/x" target="_self">x</a>').length, 0);
  assert.equal(check('<script>const a = "<a target=\\"_blank\\">";</script><p>ok</p>').length, 0);
  // A word "target" inside another attribute's expression is not a target.
  assert.equal(
    check('<button {...props} class={["slot", target && "slot--target"]}>x</button>').length,
    0,
  );
});
