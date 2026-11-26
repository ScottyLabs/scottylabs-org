// Tests for what the People page tells admins about governance: roles governance does not list,
// and the state of the last sync. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type { GovernanceSyncStatus, PeopleRoleAssignment } from "../src/lib/api/types.ts";
import { governanceNote, syncNotice } from "../src/routes/portal/people/people.ts";
import { pickerOptions } from "../src/lib/portal/ui/person-picker.ts";

const NOW = new Date("2026-11-25T17:00:00Z");

function role(overrides: Partial<PeopleRoleAssignment>): PeopleRoleAssignment {
  return {
    id: "r-1",
    role: "director",
    committee: "events",
    canPostTicker: true,
    projectIds: [],
    appointedBy: null,
    appointedAt: "2026-09-14T15:00:00Z",
    source: "portal",
    changedAt: "2026-09-14T15:00:00Z",
    changedBy: null,
    governanceListed: null,
    ...overrides,
  };
}

function status(overrides: Partial<GovernanceSyncStatus>): GovernanceSyncStatus {
  return {
    lastAttemptAt: "2026-11-25T15:00:00Z",
    lastSuccessAt: "2026-11-25T15:00:00Z",
    lastError: null,
    messages: [],
    ...overrides,
  };
}

Deno.test("roles governance does not list are marked for review", () => {
  assert.equal(governanceNote(role({ governanceListed: false })), "Not the director in governance");
  assert.equal(
    governanceNote(role({ role: "lead", governanceListed: false })),
    "Not a head in governance",
  );
  assert.equal(governanceNote(role({ governanceListed: true })), null);
  // Unknown before the first sync, for admins and for Tech leads: nothing to say.
  assert.equal(governanceNote(role({ governanceListed: null })), null);
  assert.equal(
    governanceNote(role({ role: "admin", committee: null, governanceListed: null })),
    null,
  );
  const { governanceListed: _, ...plain } = role({});
  assert.equal(governanceNote(plain), null);
});

Deno.test("the sync notice speaks up only when something needs a look", () => {
  assert.equal(syncNotice(undefined, NOW), null);
  assert.equal(syncNotice(status({}), NOW), null);

  const failed = syncNotice(
    status({
      lastError: "We couldn't reach the governance repository. Try again in a few minutes.",
    }),
    NOW,
  );
  assert.equal(failed?.tone, "warning");
  assert.equal(failed?.title, "The last governance sync did not work");
  assert.match(
    failed?.text ?? "",
    /^We couldn't reach the governance repository\. .* last synced from governance Wed, Nov 25, 10:00 AM\.$/u,
  );

  const never = syncNotice(status({ lastAttemptAt: null, lastSuccessAt: null }), NOW);
  assert.equal(never?.tone, "info");
  assert.equal(never?.text, "Directors and heads have not been synced from governance yet.");

  const stale = syncNotice(status({ lastSuccessAt: "2026-11-23T15:00:00Z" }), NOW);
  assert.equal(stale?.tone, "warning");
  assert.equal(stale?.title, "Governance roles may be out of date");
});

Deno.test("search results without sign-in history are still offered", () => {
  // Directors get no hasSignedIn; the person can still be chosen.
  const options = pickerOptions(
    [{ id: "id-helenf", andrewId: "helenf", name: "Helen Fu", initials: "HF" }],
    null,
    true,
    [],
  );
  assert.equal(options.length, 1);
  assert.equal(options[0]?.kind, "person");
});
