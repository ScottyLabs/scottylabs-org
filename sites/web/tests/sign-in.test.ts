// What the sign-in page says for each reason the service sends people back to it. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import { signInNotice } from "../src/lib/portal/sign-in-messages.ts";

function notice(query: string) {
  return signInNotice(new URLSearchParams(query));
}

Deno.test("an unreachable Keycloak is named, not reported as a generic failure", () => {
  const unavailable = notice("error=unavailable");
  assert.equal(unavailable?.tone, "warning");
  assert.match(unavailable?.message ?? "", /not responding right now/u);
  assert.notEqual(unavailable?.message, notice("error=failed")?.message);
});

Deno.test("too many sign-in attempts ask people to wait", () => {
  const limited = notice("error=rate_limited");
  assert.equal(limited?.tone, "warning");
  assert.match(limited?.message ?? "", /Wait a minute/u);
});

Deno.test("accounts that are not a CMU Andrew ID get the agreed message", () => {
  const refused = notice("error=no_andrew_id");
  assert.equal(refused?.tone, "error");
  // The apostrophe is typographic, so the pattern leaves it open.
  assert.match(
    refused?.message ?? "",
    /^We couldn.t match this account to a CMU Andrew ID\. Sign in with your Andrew account, or ask an admin for help\.$/u,
  );
});

Deno.test("an Andrew ID linked to another account points to an admin", () => {
  const linked = notice("error=linked_elsewhere");
  assert.equal(linked?.tone, "error");
  assert.match(linked?.message ?? "", /Ask an admin to reset your sign-in/u);
});

Deno.test("unknown codes still read as a failure", () => {
  assert.equal(notice("error=something_new")?.message, notice("error=failed")?.message);
});
