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
  listNames,
  presetExpiry,
  presetFor,
  shortDateTime,
  sourceOrder,
  timeLeft,
  zonedDay,
  zonedInstant,
} from "../src/lib/ticker/format.ts";
import { offersList, reservesSpace, tickerMode } from "../src/lib/ticker/mode.ts";
import type { Announcement, AnnouncementListResponse } from "../src/lib/api/types.ts";
import { composerKey, isComposerValues } from "../src/lib/ticker/portal/composer-values.ts";
import { addedMessage, appendPage } from "../src/lib/ticker/portal/paging.ts";
import { actionNote, composerRole, submitLabel } from "../src/lib/ticker/portal/wording.ts";

// Tuesday, November 24, 2026, 12:00 PM in Pittsburgh.
const NOW = new Date("2026-11-24T17:00:00Z");

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

Deno.test("one announcement stands still only while it fits", () => {
  const still = { reducedMotion: false, tooWide: false };
  assert.equal(tickerMode({ ...still, count: null }), "loading");
  assert.equal(tickerMode({ ...still, count: 0 }), "empty");
  assert.equal(tickerMode({ ...still, count: 1 }), "static");
  assert.equal(tickerMode({ ...still, count: 1, tooWide: true }), "marquee");
  assert.equal(tickerMode({ ...still, count: 5 }), "marquee");
  // Reduced motion never scrolls, even when the only announcement is cut short.
  assert.equal(tickerMode({ count: 1, reducedMotion: true, tooWide: true }), "static");
  assert.equal(tickerMode({ count: 5, reducedMotion: true, tooWide: false }), "stepper");
});

Deno.test("the full list is offered wherever text can be cut short", () => {
  assert.equal(offersList("marquee", false), true);
  assert.equal(offersList("stepper", false), true);
  assert.equal(offersList("static", false), false);
  assert.equal(offersList("static", true), true);
  assert.equal(offersList("empty", false), false);
  assert.equal(offersList("loading", true), false);
});

Deno.test("the strip holds its height unless the feed was empty last time", () => {
  assert.equal(reservesSpace(null), true);
  assert.equal(reservesSpace(3), true);
  assert.equal(reservesSpace(0), false);
});

Deno.test("the composer's button and note fit the person and the announcement", () => {
  const base = { isLive: false, isWaiting: false, manages: false, postsDirectly: false };
  assert.equal(composerRole({ ...base, postsDirectly: true }), "publish");
  assert.equal(composerRole(base), "submit");
  assert.equal(
    composerRole({ ...base, isLive: true, manages: true, postsDirectly: true }),
    "edit-live",
  );
  // A director editing a lead's submission approves it; they do not send it to an admin.
  assert.equal(
    composerRole({ ...base, isWaiting: true, manages: true, postsDirectly: true }),
    "review",
  );
  assert.equal(composerRole({ ...base, isWaiting: true }), "submit");

  assert.equal(submitLabel("publish"), "Publish announcement");
  assert.equal(submitLabel("submit"), "Send for approval");
  assert.equal(submitLabel("review"), "Save changes");
  assert.equal(submitLabel("edit-live"), "Save changes");

  assert.equal(actionNote("publish", "Events", []), null);
  assert.equal(actionNote("edit-live", "Events", ["Evan Foster"]), null);
  assert.match(
    actionNote("review", "Events", []) ?? "",
    /^Saving keeps it waiting for approval\./u,
  );
  assert.equal(
    actionNote("submit", "Events", ["Evan Foster"]),
    "Evan Foster, Director of Events, approves announcements from leads before they go live.",
  );
  assert.equal(
    actionNote("submit", "Events", []),
    "An admin approves announcements from leads in Events before they go live.",
  );
});

Deno.test("a chosen expiry is described from the current time, past midnight too", () => {
  const tomorrow = presetExpiry("tomorrow", NOW);
  // 12:30 AM the next day, with the composer still open.
  const later = new Date("2026-11-25T05:30:00Z");
  assert.equal(presetFor(tomorrow, later), null);
  assert.equal(distanceInDays(tomorrow, later), "today");
  assert.equal(presetExpiry("tomorrow", later), "2026-11-27T04:59:00.000Z");
});

function expired(id: string): Announcement {
  return {
    id,
    kind: "event",
    committee: "events",
    message: `Announcement ${id}`,
    linkUrl: null,
    state: "expired",
    startsAt: "2026-11-01T14:00:00Z",
    expiresAt: "2026-11-10T14:00:00Z",
    endedAt: null,
    position: 0,
    createdBy: null,
    approvedBy: null,
    createdAt: "2026-11-01T14:00:00Z",
    updatedAt: "2026-11-01T14:00:00Z",
    approval: null,
    canEdit: false,
    canManage: false,
  };
}

function expiredPage(
  ids: string[],
  nextBefore: string | null,
  total: number,
): AnnouncementListResponse {
  return {
    announcements: ids.map((id) => expired(id)),
    slots: { used: 2, total: 6 },
    counts: { live: 2, waiting: 0, expired: total },
    approvers: [],
    nextBefore,
  };
}

Deno.test("show more adds the next page of expired announcements once", () => {
  const first = expiredPage(["a", "b", "c"], "cursor-1", 5);
  // "c" left the ticker at the same moment as the next ones and comes again; it is not repeated.
  const second = expiredPage(["c", "d", "e"], null, 6);
  const { list, added } = appendPage(first, second);
  assert.deepEqual(
    list.announcements.map((announcement) => announcement.id),
    ["a", "b", "c", "d", "e"],
  );
  assert.deepEqual(
    added.map((announcement) => announcement.id),
    ["d", "e"],
  );
  assert.equal(list.nextBefore, null);
  assert.equal(list.counts.expired, 6);
  // The page shown before is left as it was.
  assert.equal(first.announcements.length, 3);
});

Deno.test("show more says how many it added and how many are shown", () => {
  assert.equal(addedMessage(50, 100, 120), "Added 50 more announcements. Showing 100 of 120.");
  assert.equal(addedMessage(1, 51, 51), "Added 1 more announcement. Showing all 51.");
  // The total may lag behind what is shown.
  assert.equal(addedMessage(20, 120, 118), "Added 20 more announcements. Showing all 120.");
  assert.equal(addedMessage(0, 50, 50), "No more announcements. Showing all 50.");
});

Deno.test("the composer keeps what was typed under its own key, and puts back only real values", () => {
  assert.equal(composerKey(null), "announcement:new");
  assert.equal(composerKey(null, "a1"), "announcement:new:a1");
  assert.equal(composerKey("a2"), "announcement:a2");
  assert.equal(composerKey("a2", "a1"), "announcement:a2");

  const values = {
    kind: "event",
    committee: "club",
    message: "Work sessions run every Saturday.",
    linkUrl: "",
    expiresAt: "2026-12-01T04:59:00.000Z",
  };
  assert.equal(isComposerValues(values), true);
  assert.equal(isComposerValues({ ...values, committee: "tech", expiresAt: null }), true);
  assert.equal(isComposerValues({ ...values, kind: "rumor" }), false);
  assert.equal(isComposerValues({ ...values, committee: "everyone" }), false);
  assert.equal(isComposerValues({ ...values, linkUrl: null }), false);
  assert.equal(isComposerValues({ ...values, expiresAt: 5 }), false);
  assert.equal(isComposerValues("event"), false);
});
