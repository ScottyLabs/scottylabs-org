/// <reference lib="dom" />
// Tests for restricted rich text. Run from sites/web with `deno test tests/`.
// Deno has no DOMParser, so these exercise the fallback parser that prerendering also uses.
// The DOM library reference types the browser code under test; the tests themselves never use it.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import { decodeEntities, parseHtmlFragment } from "../src/lib/editor/html-parse.ts";
import { editorInvalid, isOverLimit, limitMessage } from "../src/lib/editor/limit.ts";
import { linkFromInput, linkText } from "../src/lib/editor/links.ts";
import { floatingPosition } from "../src/lib/editor/position.ts";
import {
  normalizeRichText,
  plainText,
  plainTextLength,
  safeHref,
  sanitize,
  textToHtml,
} from "../src/lib/editor/sanitize.ts";

const LINK = 'target="_blank" rel="noopener noreferrer nofollow"';

Deno.test("sanitize keeps the allowed formatting as it is", () => {
  const html =
    "<p>Hi <strong>there</strong> <em>you</em></p><ul><li>one</li></ul><ol><li>two<br>three</li></ol>";
  assert.equal(sanitize(html), html);
  assert.equal(sanitize(""), "");
});

Deno.test("sanitize removes scripts, styles, handlers and unsafe links", () => {
  const html =
    '<p onclick="x()" style="color:red">a<script>alert(1)</script><style>p{}</style><a href="javascript:alert(1)">b</a><img src=x onerror=y></p>';
  const clean = sanitize(html);
  assert.equal(clean, "<p>ab</p>");
  for (const bad of ["script", "onclick", "style", "javascript", "<img", "alert"]) {
    assert.ok(!clean.includes(bad), `${bad} survived: ${clean}`);
  }
});

Deno.test("sanitize unwraps other tags to their text", () => {
  assert.equal(
    sanitize("<div><h2>Title</h2><span>one</span> <b>two</b> <u>three</u></div>"),
    "Titleone two three",
  );
  assert.equal(sanitize("<table><tr><td>cell</td></tr></table>"), "cell");
  assert.equal(sanitize("a <!-- note --> b"), "a  b");
});

Deno.test("links open safely and only https and mailto pass", () => {
  assert.equal(
    sanitize('<a href="https://scottylabs.org" target="_self" rel="opener">site</a>'),
    `<a href="https://scottylabs.org/" ${LINK}>site</a>`,
  );
  assert.equal(
    sanitize('<a href="mailto:hello@scottylabs.org">mail</a>'),
    `<a href="mailto:hello@scottylabs.org" ${LINK}>mail</a>`,
  );
  for (const href of [
    "/relative",
    "http://scottylabs.org",
    "javascript:alert(1)",
    "data:text/html,x",
    "  ",
    "https://",
  ]) {
    assert.equal(sanitize(`<a href="${href}">x</a>`), "x", href);
  }
  assert.equal(safeHref(" https://scottylabs.org/a?b=1&c=2 "), "https://scottylabs.org/a?b=1&c=2");
  assert.equal(safeHref("JAVASCRIPT:alert(1)"), null);
  assert.equal(safeHref("java\nscript:alert(1)"), null);
});

Deno.test("sanitize escapes text and attribute values", () => {
  assert.equal(
    sanitize("<p>1 &lt; 2 &amp;&amp; 3 &gt; 2</p>"),
    "<p>1 &lt; 2 &amp;&amp; 3 &gt; 2</p>",
  );
  assert.equal(sanitize("<p>a&nbsp;b</p>"), "<p>a&nbsp;b</p>");
  assert.equal(
    sanitize("<p>Tom&rsquo;s &#x1F436; &#169;</p>"),
    "<p>Tom\u{2019}s \u{1F436} \u{A9}</p>",
  );
  assert.equal(
    sanitize('<a href="https://x.org/?q=&quot;&gt;&lt;script&gt;">x</a>'),
    `<a href="https://x.org/?q=%22%3E%3Cscript%3E" ${LINK}>x</a>`,
  );
  assert.equal(sanitize("<p>5 < 6 and <3</p>"), "<p>5 &lt; 6 and &lt;3</p>");
});

Deno.test("nested links are flattened", () => {
  assert.equal(
    sanitize('<a href="https://a.org">a<a href="https://b.org">b</a></a>'),
    `<a href="https://a.org/" ${LINK}>a</a><a href="https://b.org/" ${LINK}>b</a>`,
  );
});

Deno.test("the fallback parser handles messy markup", () => {
  assert.equal(sanitize("<p>one<p>two"), "<p>one</p><p>two</p>");
  assert.equal(sanitize("<ul><li>a<li>b</ul>"), "<ul><li>a</li><li>b</li></ul>");
  assert.equal(sanitize("<P>Shout</P>"), "<p>Shout</p>");
  assert.equal(sanitize("<p>unclosed <strong>bold"), "<p>unclosed <strong>bold</strong></p>");
  assert.equal(sanitize("<p>broken <a href='https://x.org'"), "<p>broken </p>");
  assert.equal(sanitize("</p>stray</em>"), "stray");
  assert.equal(sanitize("<textarea><b>raw</b></textarea>"), "&lt;b&gt;raw&lt;/b&gt;");
  assert.equal(decodeEntities("&amp;&unknown;&#0;"), "&&unknown;\u{FFFD}");
  const nodes = parseHtmlFragment('<a HREF="https://x.org" href="https://y.org">x</a>');
  assert.equal(nodes[0]?.getAttribute?.("href"), "https://x.org");
});

Deno.test("plain text matches the server", () => {
  assert.equal(plainText("<p>One <strong>two</strong></p><p>three</p>"), "One two three");
  assert.equal(plainText("<ul><li>a</li><li>b</li></ul><p>c<br>d</p>"), "a b c d");
  assert.equal(plainText("<p>  spaced \n out  </p>"), "spaced out");
  assert.equal(plainText("<p>a&nbsp;&amp;&nbsp;b</p>"), "a & b");
  assert.equal(plainText("<p><script>hidden()</script>shown</p>"), "shown");
  assert.equal(plainText(""), "");
  // Code points, as Rust's chars().count().
  assert.equal(plainTextLength("<p>\u{1F436} dog</p>"), 5);
  assert.equal(plainTextLength("<p>caf\u{E9}</p>"), 4);
});

Deno.test("normalizing maps formatting from editors and other sites", () => {
  assert.equal(
    normalizeRichText("<b>bold</b> and <i>italic</i>"),
    "<p><strong>bold</strong> and <em>italic</em></p>",
  );
  assert.equal(
    normalizeRichText(
      '<span style="font-weight: 700">heavy</span> <span style="font-style:italic">slanted</span>',
    ),
    "<p><strong>heavy</strong> <em>slanted</em></p>",
  );
  // Google Docs wraps everything in a bold tag that is styled normal.
  assert.equal(
    normalizeRichText(
      '<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700;">Hi</span><span style="font-weight:400;"> there</span></p></b>',
    ),
    "<p><strong>Hi</strong> there</p>",
  );
  assert.equal(
    normalizeRichText("<h2>Heading</h2><div>Body</div>"),
    "<p><strong>Heading</strong></p><p>Body</p>",
  );
  assert.equal(
    normalizeRichText("loose <em>text</em><p>para</p>tail"),
    "<p>loose <em>text</em></p><p>para</p><p>tail</p>",
  );
});

Deno.test("normalizing tidies paragraphs, spaces and line breaks", () => {
  assert.equal(normalizeRichText("<p><br></p><p>  a  </p><p>&nbsp;</p><p></p>"), "<p>a</p>");
  assert.equal(normalizeRichText("<p><br>one<br><br></p>"), "<p>one</p>");
  assert.equal(normalizeRichText("<p>one <br> two</p>"), "<p>one<br>two</p>");
  assert.equal(
    normalizeRichText("<p>word&nbsp;word&nbsp;&nbsp;gap</p>"),
    "<p>word word&nbsp;&nbsp;gap</p>",
  );
  assert.equal(normalizeRichText("<p>\n  indented\n  text\n</p>"), "<p>indented text</p>");
  assert.equal(normalizeRichText("<p><br></p>"), "");
  assert.equal(normalizeRichText("<ul><li></li></ul>"), "");
  assert.equal(normalizeRichText(""), "");
});

Deno.test("normalizing nests marks consistently and merges runs", () => {
  assert.equal(
    normalizeRichText("<p><em><strong>both</strong></em></p>"),
    "<p><strong><em>both</em></strong></p>",
  );
  assert.equal(normalizeRichText("<p><b>a</b><b>b</b></p>"), "<p><strong>ab</strong></p>");
  assert.equal(
    normalizeRichText('<p><a href="https://x.org"><b>bold</b> link</a></p>'),
    `<p><a href="https://x.org/" ${LINK}><strong>bold</strong> link</a></p>`,
  );
  assert.equal(normalizeRichText('<p><a href="http://x.org">plain</a></p>'), "<p>plain</p>");
});

Deno.test("normalizing keeps lists and their nesting", () => {
  assert.equal(
    normalizeRichText(
      "<ul>\n  <li><p>one</p><p>more</p></li>\n  <li>two<ol><li>sub</li></ol></li>\n</ul>",
    ),
    "<ul><li>one<br>more</li><li>two<ol><li>sub</li></ol></li></ul>",
  );
  assert.equal(
    normalizeRichText("<ul><li>a</li><ul><li>nested</li></ul></ul>"),
    "<ul><li>a<ul><li>nested</li></ul></li></ul>",
  );
  assert.equal(normalizeRichText("<li>orphan</li>"), "<ul><li>orphan</li></ul>");
  assert.equal(
    normalizeRichText("<p>before</p><ol><li>x</li></ol><p>after</p>"),
    "<p>before</p><ol><li>x</li></ol><p>after</p>",
  );
});

Deno.test("normalized output passes sanitize unchanged", () => {
  const samples = [
    "<b>x</b><div>y<ul><li>z</li></ul></div>",
    '<p><a href="https://scottylabs.org/a?b=1&amp;c=2">q&amp;a</a> &lt;tag&gt;</p>',
    "<h1>Title</h1><table><tr><td>a</td><td>b</td></tr></table>",
  ];
  for (const sample of samples) {
    const normalized = normalizeRichText(sample);
    assert.equal(sanitize(normalized), normalized, sample);
    assert.equal(normalizeRichText(normalized), normalized, sample);
  }
});

Deno.test("pasted plain text becomes paragraphs", () => {
  assert.equal(
    textToHtml("One\nline two\n\nNew <para>\r\n\r\n\r\n"),
    "<p>One<br>line two</p><p>New &lt;para&gt;</p>",
  );
  assert.equal(textToHtml("   \n\n "), "");
});

Deno.test("link input accepts https and mailto and helps with the rest", () => {
  assert.deepEqual(linkFromInput("https://scottylabs.org"), {
    ok: true,
    href: "https://scottylabs.org/",
  });
  assert.deepEqual(linkFromInput(" scottylabs.org/projects "), {
    ok: true,
    href: "https://scottylabs.org/projects",
  });
  assert.deepEqual(linkFromInput("hello@scottylabs.org"), {
    ok: true,
    href: "mailto:hello@scottylabs.org",
  });
  assert.deepEqual(linkFromInput("mailto:hello@scottylabs.org"), {
    ok: true,
    href: "mailto:hello@scottylabs.org",
  });
  assert.equal(linkFromInput("").ok, false);
  assert.equal(linkFromInput("http://scottylabs.org").ok, false);
  assert.equal(linkFromInput("ftp://scottylabs.org").ok, false);
  assert.equal(linkFromInput("javascript:alert(1)").ok, false);
  assert.equal(linkFromInput("not a link").ok, false);
  assert.equal(linkFromInput("localhost").ok, false);
  assert.equal(linkText("https://scottylabs.org/"), "scottylabs.org");
  assert.equal(linkText("mailto:hello@scottylabs.org"), "hello@scottylabs.org");
});

Deno.test("the floating bar stays above the selection and inside the viewport", () => {
  const viewport = { width: 1000, height: 800 };
  const bar = { width: 200, height: 34 };
  assert.deepEqual(
    floatingPosition({ top: 300, left: 400, width: 100, height: 20 }, bar, viewport),
    {
      top: 258,
      left: 350,
      placement: "above",
    },
  );
  assert.deepEqual(floatingPosition({ top: 20, left: 0, width: 50, height: 20 }, bar, viewport), {
    top: 48,
    left: 8,
    placement: "below",
  });
  assert.equal(
    floatingPosition({ top: 300, left: 980, width: 10, height: 20 }, bar, viewport).left,
    792,
  );
});

Deno.test("text over the limit is invalid and says what to do", () => {
  assert.ok(!isOverLimit(40, 40));
  assert.ok(isOverLimit(52, 40));
  assert.ok(!isOverLimit(52));
  assert.equal(limitMessage(4000), "Shorten this to 4,000 characters or fewer.");
  assert.equal(limitMessage(1), "Shorten this to 1 character or fewer.");
});

Deno.test("a parent cannot hide text over the limit by passing invalid={false}", () => {
  assert.ok(editorInvalid({ invalid: false, over: true, fieldInvalid: false }));
  assert.ok(editorInvalid({ over: true }));
  assert.ok(editorInvalid({ invalid: true, over: false }));
  assert.ok(editorInvalid({ over: false, fieldInvalid: true }));
  assert.ok(!editorInvalid({ invalid: false, over: false, fieldInvalid: false }));
  assert.ok(!editorInvalid({ over: false }));
});
