// Tests for the person picker's helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  isExcluded,
  normalizeAndrewId,
  personInitials,
  personName,
} from "../src/lib/portal/ui/person-picker.ts";

Deno.test("Andrew IDs follow the server's rules", () => {
  assert.equal(normalizeAndrewId(" TKanz "), "tkanz");
  assert.equal(normalizeAndrewId("a.b_c-1"), "a.b_c-1");
  assert.equal(normalizeAndrewId("Helen Fu"), null);
  assert.equal(normalizeAndrewId(".dot"), null);
  assert.equal(normalizeAndrewId(""), null);
  assert.equal(normalizeAndrewId("a".repeat(65)), null);
});

Deno.test("picked people show a name and initials, and can be excluded", () => {
  const known = { id: "u1", andrewId: "hfu", name: "Helen Fu", initials: "HF" };
  assert.equal(personName(known), "Helen Fu");
  assert.equal(personInitials(known), "HF");
  assert.equal(personName({ andrewId: "newbie" }), "newbie");
  assert.equal(personInitials({ andrewId: "newbie" }), "ne");
  assert.ok(isExcluded(known, ["u1"]));
  assert.ok(isExcluded(known, ["HFU"]));
  assert.ok(isExcluded({ andrewId: "newbie" }, ["newbie"]));
  assert.ok(!isExcluded(known, ["u2", "other"]));
});
