// Tests for the last good copies public pages show when their content cannot load. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  type CopyStore,
  MAX_AGE_MS,
  forget,
  isRecord,
  lastGoodMessage,
  recall,
  remember,
} from "../src/lib/site/last-good.ts";

function memoryStore(): CopyStore & { items: Map<string, string> } {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
  };
}

const isList = (value: unknown): value is { id: string }[] =>
  Array.isArray(value) && value.every((item) => isRecord(item) && typeof item.id === "string");

const NOW = Date.UTC(2026, 10, 25, 15, 0);

Deno.test("a copy comes back as it was saved, with when", () => {
  const store = memoryStore();
  remember("projects", [{ id: "a" }, { id: "b" }], store, NOW - 1000);
  assert.deepEqual(recall("projects", isList, store, NOW), {
    value: [{ id: "a" }, { id: "b" }],
    savedAt: NOW - 1000,
  });
  assert.equal(recall("committees", isList, store, NOW), null);
});

Deno.test("copies older than a week, from the future or of another shape are not used", () => {
  const store = memoryStore();
  remember("old", [{ id: "a" }], store, NOW - MAX_AGE_MS - 1);
  assert.equal(recall("old", isList, store, NOW), null);
  // An expired copy is dropped.
  assert.equal(store.items.size, 0);

  remember("ahead", [{ id: "a" }], store, NOW + 60_000);
  assert.equal(recall("ahead", isList, store, NOW), null);

  remember("shape", { committees: [] }, store, NOW);
  assert.equal(recall("shape", isList, store, NOW), null);

  store.setItem("sl-last-good:broken", "{not json");
  assert.equal(recall("broken", isList, store, NOW), null);
});

Deno.test("large answers are not kept and content taken down is forgotten", () => {
  const store = memoryStore();
  remember("big", [{ id: "x".repeat(600 * 1024) }], store, NOW);
  assert.equal(recall("big", isList, store, NOW), null);

  remember("project:nova", [{ id: "a" }], store, NOW);
  forget("project:nova", store);
  assert.equal(recall("project:nova", isList, store, NOW), null);
});

Deno.test("blocked storage never throws", () => {
  const blocked: CopyStore = {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("quota");
    },
    removeItem: () => {
      throw new Error("blocked");
    },
  };
  remember("projects", [], blocked, NOW);
  assert.equal(recall("projects", isList, blocked, NOW), null);
  forget("projects", blocked);
  assert.equal(recall("projects", isList, null, NOW), null);
});

Deno.test("the note says when the copy is from", () => {
  assert.equal(
    lastGoodMessage(null, NOW),
    "We couldn\u2019t refresh this page, so you\u2019re seeing what loaded before.",
  );
  assert.equal(
    lastGoodMessage(NOW - 60_000, NOW),
    "We couldn\u2019t reach the site, so this is how the page looked when you visited earlier today.",
  );
  assert.match(
    lastGoodMessage(NOW - 3 * 24 * 60 * 60_000, NOW),
    /when you visited on November 2\d\.$/u,
  );
});
