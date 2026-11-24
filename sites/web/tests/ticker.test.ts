// Tests for the ticker's plain TypeScript helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  addDays,
  daysBetween,
  distanceInDays,
  expiryProblem,
  expirySentence,
  latestExpiry,
  leavesLabel,
  linkHost,
  listNames,
  presetExpiry,
  presetFor,
  shortDateTime,
  sourceOrder,
  timeLeft,
  zonedDay,
  zonedInstant,
} from "../src/lib/ticker/format.ts";

// Tuesday, November 24, 2026, 12:00 PM in Pittsburgh.
const NOW = new Date("2026-11-24T17:00:00Z");

Deno.test("link hosts drop the scheme, www and path", () => {
  assert.equal(linkHost("https://www.collegecart.org/shop?x=1"), "collegecart.org");
  assert.equal(linkHost("https://maps.scottylabs.org"), "maps.scottylabs.org");
  assert.equal(linkHost(null), null);
  assert.equal(linkHost(""), null);
  assert.equal(linkHost("not a link"), null);
});

Deno.test("sources list committees first and ScottyLabs last", () => {
  assert.deepEqual(sourceOrder(["club", "events", "tech"]), ["events", "tech", "club"]);
  assert.deepEqual(sourceOrder(["events"]), ["events"]);
});

Deno.test("calendar days are counted in Pittsburgh", () => {
  assert.equal(zonedDay(NOW), "2026-11-24");
  // 11 PM in Pittsburgh is already the next day in UTC.
  assert.equal(zonedDay("2026-11-25T04:00:00Z"), "2026-11-24");
  assert.equal(addDays("2026-11-24", 7), "2026-12-01");
  assert.equal(addDays("2026-12-31", 1), "2027-01-01");
  assert.equal(daysBetween("2026-11-24", "2026-12-01"), 7);
  assert.equal(zonedInstant("2026-12-01", "09:00"), "2026-12-01T14:00:00.000Z");
  // Daylight saving time: 9 AM in July is 13:00 UTC.
  assert.equal(zonedInstant("2026-07-01", "09:00"), "2026-07-01T13:00:00.000Z");
});

Deno.test("presets end at 11:59 PM that many days out", () => {
  assert.equal(presetExpiry("tomorrow", NOW), "2026-11-26T04:59:00.000Z");
  assert.equal(presetExpiry("1w", NOW), "2026-12-02T04:59:00.000Z");
  assert.equal(presetExpiry("2w", NOW), "2026-12-09T04:59:00.000Z");
  assert.equal(presetFor("2026-12-02T04:59:00.000Z", NOW), "1w");
  assert.equal(presetFor("2026-12-01T14:00:00.000Z", NOW), null);
  assert.equal(presetFor(null, NOW), null);
});

Deno.test("expiry sentences say exactly when it leaves", () => {
  assert.equal(
    expirySentence(presetExpiry("1w", NOW), NOW),
    "Leaves the ticker on Tue, Dec 1 at 11:59 PM, 7 days from now.",
  );
  assert.equal(distanceInDays(presetExpiry("tomorrow", NOW), NOW), "tomorrow");
  assert.equal(distanceInDays("2026-11-24T22:00:00Z", NOW), "today");
  assert.equal(shortDateTime("2026-12-01T14:00:00Z", NOW), "Tue, Dec 1, 9:00 AM");
});

Deno.test("expiries must be in the next 30 days", () => {
  assert.equal(expiryProblem(presetExpiry("2w", NOW), NOW), null);
  assert.equal(expiryProblem(latestExpiry(NOW).toISOString(), NOW), null);
  assert.match(expiryProblem("2026-11-24T16:00:00Z", NOW) ?? "", /future/u);
  assert.match(expiryProblem("2026-12-25T18:00:00Z", NOW) ?? "", /30 days/u);
  assert.match(expiryProblem(null, NOW) ?? "", /Pick when/u);
});

Deno.test("time left is a share of the announcement's time on the ticker", () => {
  const half = timeLeft(
    {
      startsAt: "2026-11-20T17:00:00Z",
      expiresAt: "2026-11-28T17:00:00Z",
      createdAt: "2026-11-20T17:00:00Z",
    },
    NOW,
  );
  assert.equal(half.label, "in 4 days");
  assert.equal(half.fraction, 0.5);
  assert.equal(half.soon, false);
  const soon = timeLeft(
    { startsAt: null, expiresAt: "2026-11-26T17:00:00Z", createdAt: "2026-11-14T17:00:00Z" },
    NOW,
  );
  assert.equal(soon.soon, true);
  assert.equal(soon.fraction, 2 / 12);
  assert.equal(leavesLabel("2026-11-30T17:00:00Z", NOW), "Leaves in 6 days");
  assert.equal(leavesLabel("2026-11-22T17:00:00Z", NOW), "Left 2 days ago");
});

Deno.test("names are joined into a sentence", () => {
  assert.equal(listNames(["Evan Foster"]), "Evan Foster");
  assert.equal(listNames(["Evan Foster", "Thomas Kanz"]), "Evan Foster and Thomas Kanz");
  assert.equal(listNames(["A", "B", "C"]), "A, B and C");
  assert.equal(listNames([]), "");
});
