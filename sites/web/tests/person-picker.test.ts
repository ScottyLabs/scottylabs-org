// Tests for the person picker's helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  type FoundPerson,
  choosableOption,
  isExcluded,
  normalizeAndrewId,
  personInitials,
  personName,
  pickerOptions,
  resultsAreCurrent,
} from "../src/lib/portal/ui/person-picker.ts";

function found(andrewId: string, name: string): FoundPerson {
  return { id: `id-${andrewId}`, andrewId, name, initials: name.slice(0, 2), hasSignedIn: true };
}

function keys(options: readonly { key: string }[]): string[] {
  return options.map((option) => option.key);
}

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

Deno.test("results count as current only once the search for the text as typed has answered", () => {
  assert.ok(resultsAreCurrent(" tkanz ", "tkanz", false));
  assert.ok(!resultsAreCurrent("tkanz2", "t", false));
  assert.ok(!resultsAreCurrent("tkanz", "tkanz", true));
  assert.ok(!resultsAreCurrent("t", "", true));
});

Deno.test("the Andrew ID row waits for the search and skips people already found or excluded", () => {
  const people = [found("tkanz", "Thomas Kanz"), found("evanfost", "Evan Foster")];
  assert.deepEqual(keys(pickerOptions(people, "tkanz2", true, [])), [
    "person-id-tkanz",
    "person-id-evanfost",
    "andrew-tkanz2",
  ]);
  // Still waiting for the search: no Andrew ID row yet.
  assert.deepEqual(keys(pickerOptions(people, "tkanz2", false, [])), [
    "person-id-tkanz",
    "person-id-evanfost",
  ]);
  // Already in the results, so it is not offered twice.
  assert.deepEqual(keys(pickerOptions(people, "tkanz", true, [])), [
    "person-id-tkanz",
    "person-id-evanfost",
  ]);
  // Excluded people and Andrew IDs are left out.
  assert.deepEqual(keys(pickerOptions(people, "newbie", true, ["id-tkanz", "NEWBIE"])), [
    "person-id-evanfost",
  ]);
  assert.deepEqual(keys(pickerOptions([], null, true, [])), []);
});

Deno.test("Enter and clicks never choose results left over from an earlier search", () => {
  // The list for "t" is on screen, then "tkanz2" is typed and Enter pressed before it answers.
  const earlier = pickerOptions([found("tkanz", "Thomas Kanz")], "tkanz2", false, []);
  const typedFast = resultsAreCurrent("tkanz2", "t", true);
  assert.equal(choosableOption(earlier, 0, typedFast), undefined);

  const answered = pickerOptions([], "tkanz2", true, []);
  const option = choosableOption(answered, 0, resultsAreCurrent("tkanz2", "tkanz2", false));
  assert.deepEqual(option, { kind: "andrew_id", key: "andrew-tkanz2", andrewId: "tkanz2" });
  assert.equal(choosableOption(answered, 3, true), undefined);
  assert.equal(choosableOption(answered, -1, true), undefined);
});
