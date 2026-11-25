// Tests for the public site's plain TypeScript helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import { INJECTED_ATTRIBUTE, removeInjectedHeadTags } from "../src/lib/site/head.ts";
import { linkHost } from "../src/lib/site/links.ts";

Deno.test("only the tags the service injected leave the head", () => {
  const head = [
    { name: "charset", injected: false, removed: false },
    { name: "title", injected: true, removed: false },
    { name: "description", injected: true, removed: false },
    { name: "og:image", injected: true, removed: false },
    { name: "stylesheet", injected: false, removed: false },
  ];
  const asked: string[] = [];
  removeInjectedHeadTags({
    querySelectorAll(selectors: string) {
      asked.push(selectors);
      return head
        .filter((tag) => tag.injected)
        .map((tag) => ({
          remove() {
            tag.removed = true;
          },
        }));
    },
  });
  assert.deepEqual(asked, [`[${INJECTED_ATTRIBUTE}]`]);
  assert.deepEqual(
    head.filter((tag) => tag.removed).map((tag) => tag.name),
    ["title", "description", "og:image"],
  );
});

Deno.test("the attribute matches the one the site service writes", () => {
  // services/site/src/static_files.rs INJECTED_ATTRIBUTE
  assert.equal(INJECTED_ATTRIBUTE, "data-sl-injected");
});

Deno.test("link hosts drop the scheme, www, port and path", () => {
  assert.equal(linkHost("https://www.collegecart.org/shop?x=1"), "collegecart.org");
  assert.equal(linkHost("https://maps.scottylabs.org"), "maps.scottylabs.org");
  assert.equal(linkHost("https://www.tartanhacks.com/volunteer"), "tartanhacks.com");
  assert.equal(linkHost("https://example.com:8443/x"), "example.com");
  assert.equal(linkHost(" https://cmumaps.com "), "cmumaps.com");
  assert.equal(linkHost("http://example.com"), "example.com");
  for (const empty of [
    null,
    undefined,
    "",
    "  ",
    "not a link",
    "javascript:alert(1)",
    "mailto:hi@example.com",
  ]) {
    assert.equal(linkHost(empty), null, String(empty));
  }
});

Deno.test("https-only link hosts leave out plain http", () => {
  assert.equal(linkHost("https://cmumaps.com", { httpsOnly: true }), "cmumaps.com");
  assert.equal(linkHost("http://example.com", { httpsOnly: true }), null);
  assert.equal(linkHost("javascript:alert(1)", { httpsOnly: true }), null);
});
