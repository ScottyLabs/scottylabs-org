// Tests for the portal's plain TypeScript helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  ApiError,
  apiUrl,
  errorFromResponse,
  get,
  isAbortError,
  onUnauthenticated,
  post,
  setCsrfToken,
} from "../src/lib/api/client.ts";
import type { RoleAssignment } from "../src/lib/api/types.ts";
import { committeeNames, isCommitteeSlug, isTickerSource } from "../src/lib/portal/committees.ts";
import {
  formatAbsolute,
  formatDateTimeLong,
  formatDay,
  formatRelative,
  formatShortDate,
  formatTime,
  formatUpdated,
  fromZonedInputValue,
  partOfDay,
  toZonedInputValue,
} from "../src/lib/portal/dates.ts";
import { safeReturnTo, signInHref } from "../src/lib/portal/paths.ts";
import { describeRoles, firstName, primaryRoleLabel } from "../src/lib/portal/roles.ts";
import { signInNotice } from "../src/lib/portal/sign-in-messages.ts";

const NOW = "2026-11-24T17:00:00Z";

Deno.test("dates are shown in Pittsburgh time", () => {
  assert.equal(formatAbsolute("2026-12-01T14:00:00Z", NOW), "Tue, Dec 1, 9:00 AM");
  assert.equal(formatAbsolute("2027-01-05T14:00:00Z", NOW), "Tue, Jan 5, 2027, 9:00 AM");
  assert.equal(formatDateTimeLong("2026-12-04T22:00:00Z", NOW), "Fri, Dec 4 at 5:00 PM");
  assert.equal(formatDay(NOW, NOW), "Tue, Nov 24");
  assert.equal(formatShortDate("2026-11-20T15:00:00Z", NOW), "Nov 20");
  assert.equal(formatShortDate("2025-11-20T15:00:00Z", NOW), "Nov 20, 2025");
  assert.equal(formatTime("2026-07-04T16:05:00Z"), "12:05 PM");
  assert.equal(formatAbsolute("not a date"), "");
});

Deno.test("relative times read naturally", () => {
  const at = (ms: number) => new Date(Date.parse(NOW) + ms);
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;
  assert.equal(formatRelative(at(20_000), NOW), "just now");
  assert.equal(formatRelative(at(-5 * minute), NOW), "5 min ago");
  assert.equal(formatRelative(at(-2 * hour), NOW), "2 hours ago");
  assert.equal(formatRelative(at(hour), NOW), "in 1 hour");
  assert.equal(formatRelative(at(day), NOW), "tomorrow");
  assert.equal(formatRelative(at(-day), NOW), "yesterday");
  assert.equal(formatRelative(at(6 * day), NOW), "in 6 days");
  assert.equal(formatRelative(at(-60 * day), NOW), "2 months ago");
  assert.equal(formatUpdated(at(-10_000), NOW), "Just now");
  assert.equal(formatUpdated(at(-2 * minute), NOW), "2 min ago");
  assert.equal(formatUpdated("2026-11-20T15:00:00Z", NOW), "Nov 20");
});

Deno.test("greetings follow the Pittsburgh clock", () => {
  assert.equal(partOfDay("2026-11-24T14:00:00Z"), "morning");
  assert.equal(partOfDay("2026-11-24T19:00:00Z"), "afternoon");
  assert.equal(partOfDay("2026-11-25T01:00:00Z"), "evening");
});

Deno.test("datetime-local values convert through Pittsburgh time", () => {
  assert.equal(fromZonedInputValue("2026-12-01T09:00"), "2026-12-01T14:00:00.000Z");
  assert.equal(fromZonedInputValue("2026-07-04T12:00"), "2026-07-04T16:00:00.000Z");
  assert.equal(toZonedInputValue("2026-12-01T14:00:00Z"), "2026-12-01T09:00");
  assert.equal(toZonedInputValue("2026-07-04T16:00:00Z"), "2026-07-04T12:00");
  // Around the switch to and from daylight saving time.
  assert.equal(fromZonedInputValue("2026-03-08T03:30"), "2026-03-08T07:30:00.000Z");
  assert.equal(fromZonedInputValue("2026-11-01T00:30"), "2026-11-01T04:30:00.000Z");
  assert.equal(fromZonedInputValue("2026-11-01T03:00"), "2026-11-01T08:00:00.000Z");
  assert.equal(fromZonedInputValue("tomorrow"), null);
  for (const iso of ["2026-01-15T05:45:00.000Z", "2026-06-30T23:59:00.000Z"]) {
    assert.equal(fromZonedInputValue(toZonedInputValue(iso)), iso);
  }
});

Deno.test("returnTo stays on portal paths", () => {
  assert.equal(safeReturnTo("/portal/ticker/?tab=live#new"), "/portal/ticker/?tab=live#new");
  assert.equal(safeReturnTo("/portal"), "/portal");
  for (const unsafe of [
    null,
    "",
    "/",
    "/team/",
    "//evil.example/portal/",
    "https://evil.example/portal/",
    "/portal\\..\\evil",
    "/portal/../team/",
    "/portalx",
    "/portal/sign-in/",
    "/portal/sign-in",
    " /portal/",
    "/portal/\nx",
    `/portal/${"a".repeat(3000)}`,
  ]) {
    assert.equal(safeReturnTo(unsafe), "/portal/", String(unsafe));
  }
  assert.equal(
    signInHref("/portal/projects/", { reason: "session_ended" }),
    "/portal/sign-in/?returnTo=%2Fportal%2Fprojects%2F&reason=session_ended",
  );
});

Deno.test("sign-in notices match the redirect", () => {
  assert.equal(signInNotice(new URLSearchParams("error=cancelled"))?.tone, "warning");
  assert.equal(signInNotice(new URLSearchParams("error=nonsense"))?.tone, "error");
  assert.equal(signInNotice(new URLSearchParams("signedOut=1"))?.message, "You are signed out.");
  assert.equal(signInNotice(new URLSearchParams("reason=session_ended"))?.tone, "info");
  assert.equal(signInNotice(new URLSearchParams("")), null);
});

function role(partial: Partial<RoleAssignment> & Pick<RoleAssignment, "role">): RoleAssignment {
  return {
    id: crypto.randomUUID(),
    committee: null,
    canPostTicker: false,
    projectIds: [],
    appointedBy: null,
    appointedAt: NOW,
    source: "portal",
    ...partial,
  };
}

Deno.test("roles are described in plain words", () => {
  assert.equal(describeRoles([role({ role: "admin" })]), "Admin, exec board");
  assert.equal(
    describeRoles([
      role({ role: "director", committee: "events" }),
      role({ role: "lead", committee: "tech" }),
    ]),
    "Director of Events, lead in Tech",
  );
  assert.equal(describeRoles([]), "");
  assert.equal(primaryRoleLabel([role({ role: "lead", committee: "design" })]), "Lead in Design");
  assert.equal(primaryRoleLabel([]), null);
  assert.equal(firstName({ name: "Thomas Kanz", andrewId: "tkanz" }), "Thomas");
  assert.equal(firstName({ name: "  ", andrewId: "tkanz" }), "tkanz");
  assert.equal(committeeNames(["events", "tech", "design"]), "Events, Tech and Design");
  assert.ok(isCommitteeSlug("labrador"));
  assert.ok(!isCommitteeSlug("club"));
  assert.ok(isTickerSource("club"));
});

Deno.test("API URLs stay under /api on this origin", () => {
  assert.equal(apiUrl("/me"), "/api/me");
  assert.equal(
    apiUrl("/projects", { committee: "tech", q: "a b", state: null, ids: ["1", "2"] }),
    "/api/projects?committee=tech&q=a+b&ids=1&ids=2",
  );
  assert.throws(() => apiUrl("//evil.example/x"));
  assert.throws(() => apiUrl("me"));
});

Deno.test("error bodies become ApiErrors", async () => {
  const body = {
    error: {
      code: "validation_failed",
      message: "Write a message of 90 characters or fewer.",
      fields: { message: "Write a message of 90 characters or fewer.", bad: 3 },
    },
  };
  const error = await errorFromResponse(Response.json(body, { status: 422 }));
  assert.equal(error.status, 422);
  assert.equal(error.code, "validation_failed");
  assert.equal(error.message, "Write a message of 90 characters or fewer.");
  assert.deepEqual(error.fields, { message: "Write a message of 90 characters or fewer." });

  const proxy = await errorFromResponse(new Response("<html>Bad gateway</html>", { status: 502 }));
  assert.equal(proxy.code, "internal");
  assert.match(proxy.message, /Try again/u);
});

interface Call {
  url: string;
  init: RequestInit;
}

function stubFetch(response: () => Response | Promise<Response>, calls: Call[]): typeof fetch {
  return (input, init) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    calls.push({ url, init: init ?? {} });
    return Promise.resolve(response());
  };
}

Deno.test("writes carry the CSRF token and reads do not", async () => {
  const calls: Call[] = [];
  const ok = stubFetch(() => Response.json({ ok: true }), calls);
  setCsrfToken("token-123");
  try {
    await get("/me", { fetch: ok });
    await post("/announcements", { message: "Hi" }, { fetch: ok });
  } finally {
    setCsrfToken(null);
  }
  const [read, write] = calls;
  assert.equal(new Headers(read?.init.headers).get("X-CSRF-Token"), null);
  assert.equal(new Headers(write?.init.headers).get("X-CSRF-Token"), "token-123");
  assert.equal(new Headers(write?.init.headers).get("Content-Type"), "application/json");
  assert.equal(write?.init.body, JSON.stringify({ message: "Hi" }));
  assert.equal(write?.init.credentials, "same-origin");
});

Deno.test("401 responses reach the sign-in handler unless the call opts out", async () => {
  const calls: Call[] = [];
  const unauthorized = stubFetch(
    () =>
      Response.json({ error: { code: "unauthenticated", message: "Sign in." } }, { status: 401 }),
    calls,
  );
  let seen = 0;
  const stop = onUnauthenticated(() => {
    seen += 1;
  });
  try {
    await assert.rejects(get("/overview", { fetch: unauthorized }), ApiError);
    await assert.rejects(
      get("/me", { fetch: unauthorized, redirectOnUnauthenticated: false }),
      ApiError,
    );
  } finally {
    stop();
  }
  assert.equal(seen, 1);
});

const failingFetch: typeof fetch = () => Promise.reject(new TypeError("Failed to fetch"));
const abortedFetch: typeof fetch = () => Promise.reject(new DOMException("Aborted", "AbortError"));

Deno.test("network failures and empty answers", async () => {
  const error = await get("/me", { fetch: failingFetch }).catch((caught: unknown) => caught);
  assert.ok(error instanceof ApiError);
  assert.equal(error.code, "network");
  assert.equal(error.status, 0);

  const abortError = await get("/me", { fetch: abortedFetch }).catch((caught: unknown) => caught);
  assert.ok(isAbortError(abortError));

  const calls: Call[] = [];
  const empty = stubFetch(() => new Response(null, { status: 204 }), calls);
  const result: unknown = await post<unknown>("/auth/logout", undefined, { fetch: empty });
  assert.equal(result, undefined);
});
