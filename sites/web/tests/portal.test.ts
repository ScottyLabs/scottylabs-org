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
  put,
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
import {
  type AccessState,
  destinationAfterSignIn,
  isNoAccessPath,
  portalRedirect,
} from "../src/lib/portal/access.ts";
import {
  KEPT_FOR_MS,
  type KeptListStore,
  type KeptStore,
  clearKeptChanges,
  forgetChanges,
  keepChanges,
  keptChanges,
  restorePlan,
} from "../src/lib/portal/kept-changes.ts";
import { replaceLocation } from "../src/lib/portal/navigation.ts";
import { safeReturnTo, signInHref } from "../src/lib/portal/paths.ts";
import { describeRoles, firstName, primaryRoleLabel } from "../src/lib/portal/roles.ts";
import { signInNotice } from "../src/lib/portal/sign-in-messages.ts";
import { endSessionForm, endSessionUrl } from "../src/lib/portal/sign-out.ts";
import { focusAfterDialog } from "../src/lib/portal/ui/focus.ts";

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

Deno.test("people without a role go straight to the no-access page after signing in", () => {
  assert.equal(
    destinationAfterSignIn(true, "/portal/projects/?tab=mine"),
    "/portal/projects/?tab=mine",
  );
  assert.equal(destinationAfterSignIn(true, "https://evil.example/"), "/portal/");
  assert.equal(destinationAfterSignIn(false, "/portal/projects/"), "/portal/no-access/");
  assert.equal(destinationAfterSignIn(false, "/portal/"), "/portal/no-access/");
});

const at = (path: string) => new URL(path, "https://scottylabs.org");

const ready = (canUsePortal: boolean): AccessState => ({
  status: "ready",
  signedOutReason: null,
  canUsePortal,
});

const signedOut = (reason: AccessState["signedOutReason"]): AccessState => ({
  status: "signed_out",
  signedOutReason: reason,
  canUsePortal: false,
});

Deno.test("the portal shell sends people where their session allows", () => {
  assert.equal(portalRedirect(at("/portal/ticker/"), ready(true)), null);
  assert.equal(portalRedirect(at("/portal/no-access/"), ready(true)), "/portal/");
  assert.equal(portalRedirect(at("/portal/no-access"), ready(true)), "/portal/");
  assert.equal(portalRedirect(at("/portal/?x=1"), ready(false)), "/portal/no-access/");
  assert.equal(portalRedirect(at("/portal/no-access/"), ready(false)), null);
  assert.equal(
    portalRedirect(at("/portal/projects/?tab=mine"), signedOut("not_signed_in")),
    "/portal/sign-in/?returnTo=%2Fportal%2Fprojects%2F%3Ftab%3Dmine",
  );
  assert.equal(
    portalRedirect(at("/portal/ticker/"), signedOut("expired")),
    "/portal/sign-in/?returnTo=%2Fportal%2Fticker%2F&reason=session_ended",
  );
  // Signing out navigates on its own, and the sign-in page needs no session.
  assert.equal(portalRedirect(at("/portal/"), signedOut("signed_out")), null);
  assert.equal(portalRedirect(at("/portal/sign-in/"), signedOut("not_signed_in")), null);
  assert.equal(portalRedirect(at("/portal/sign-in/"), ready(false)), null);
  // Nothing moves while the session is still loading or could not be checked.
  for (const status of ["idle", "loading", "error"] as const) {
    const state: AccessState = { status, signedOutReason: null, canUsePortal: false };
    assert.equal(portalRedirect(at("/portal/ticker/"), state), null, status);
  }
  assert.ok(isNoAccessPath("/portal/no-access"));
  assert.ok(!isNoAccessPath("/portal/no-access/x"));
});

Deno.test("leaving a page always gets the person there", async () => {
  const loads: string[] = [];
  const load = (href: string) => {
    loads.push(href);
  };

  // The router got there.
  let here = "/portal/sign-in/";
  await replaceLocation(
    "/portal/",
    () => here === "/portal/sign-in/",
    (href) => {
      here = href;
      return Promise.resolve();
    },
    load,
  );
  assert.deepEqual(loads, []);

  // The router has no route for the path, so the browser loads it in full.
  await replaceLocation(
    "/portal/old-bookmark/",
    () => true,
    () => Promise.reject(new Error("navigation_route_missing")),
    load,
  );
  assert.deepEqual(loads, ["/portal/old-bookmark/"]);

  // A newer navigation cut this one short without an error, and the person still has to go.
  await replaceLocation(
    "/portal/no-access/",
    () => true,
    () => Promise.resolve(),
    load,
  );
  assert.deepEqual(loads, ["/portal/old-bookmark/", "/portal/no-access/"]);

  // A newer navigation made this one unnecessary.
  await replaceLocation(
    "/portal/",
    () => false,
    () => Promise.reject(new Error("x")),
    load,
  );
  assert.equal(loads.length, 2);
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

Deno.test("only the service's own body says sign-in is not set up", async () => {
  const outage = await errorFromResponse(
    new Response("<html>Service unavailable</html>", { status: 503 }),
  );
  assert.equal(outage.code, "internal");
  assert.match(outage.message, /Try again/u);

  const notConfigured = await errorFromResponse(
    Response.json(
      { error: { code: "auth_not_configured", message: "Sign-in is not set up yet." } },
      { status: 503 },
    ),
  );
  assert.equal(notConfigured.code, "auth_not_configured");
  assert.equal(notConfigured.message, "Sign-in is not set up yet.");
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

function refused(): Response {
  return Response.json(
    { error: { code: "forbidden", message: "Your session check failed." } },
    { status: 403 },
  );
}

Deno.test("a write refused for an outdated CSRF token is sent once more with the current one", async () => {
  const calls: Call[] = [];
  // A tab loaded before a deploy changed how the token is made.
  const server = stubFetch(() => {
    const last = calls.at(-1);
    if (last?.url === "/api/me") return Response.json({ csrfToken: "token-new" });
    const sent = new Headers(last?.init.headers).get("X-CSRF-Token");
    return sent === "token-new" ? Response.json({ saved: true }) : refused();
  }, calls);
  setCsrfToken("token-old");
  try {
    const saved = await put<{ saved: boolean }>(
      "/projects/p1/draft",
      { doc: {} },
      { fetch: server, query: { x: 1 } },
    );
    assert.deepEqual(saved, { saved: true });
    assert.deepEqual(
      calls.map((call) => [call.init.method, call.url]),
      [
        ["PUT", "/api/projects/p1/draft?x=1"],
        ["GET", "/api/me"],
        ["PUT", "/api/projects/p1/draft?x=1"],
      ],
    );
    // A refusal for any other reason is not sent again: the token was already the current one.
    calls.length = 0;
    const stillRefused = stubFetch(() => {
      if (calls.at(-1)?.url === "/api/me") return Response.json({ csrfToken: "token-new" });
      return refused();
    }, calls);
    await assert.rejects(post("/announcements", {}, { fetch: stillRefused }), ApiError);
    assert.equal(calls.length, 2);
  } finally {
    setCsrfToken(null);
  }
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

Deno.test("a closing dialog gives focus back to its opener, or to the page heading when that is gone", () => {
  const body = { isConnected: true };
  const opener = { isConnected: true };
  assert.equal(focusAfterDialog(opener, body, true), "opener");
  assert.equal(focusAfterDialog(opener, body, false), "opener");
  // Opened on arrival, as for the overview's Add project, focus was on the body or on a link the navigation removed.
  assert.equal(focusAfterDialog(body, body, true), "heading");
  assert.equal(focusAfterDialog(null, body, true), "heading");
  assert.equal(focusAfterDialog({ isConnected: false }, body, true), "heading");
  // The page already moved focus on closing, so it stays where the page put it.
  assert.equal(focusAfterDialog({ isConnected: false }, body, false), null);
});

/** A stand-in for sessionStorage. */
function memoryStore(): KeptStore & { items: Map<string, string> } {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
  };
}

/** A stand-in for sessionStorage that can list its keys, in insertion order as browsers do. */
function listStore(): KeptListStore & { items: Map<string, string> } {
  const items = new Map<string, string>();
  return {
    items,
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
    key: (index) => [...items.keys()][index] ?? null,
    get length() {
      return items.size;
    },
  };
}

const isText = (value: unknown): value is string => typeof value === "string";

Deno.test("work kept when the session ends comes back only to the person who made it", () => {
  const store = memoryStore();
  const noon = Date.UTC(2026, 10, 25, 12);
  keepChanges("project:p1", { userId: "u1", base: "v1", value: "Edited" }, noon, store);

  assert.deepEqual(keptChanges("project:p1", "u1", isText, noon + 60_000, store), {
    userId: "u1",
    base: "v1",
    value: "Edited",
    keptAt: noon,
  });
  // Nothing under another key.
  assert.equal(keptChanges("project:p2", "u1", isText, noon, store), null);
  // Someone else signing in to this tab neither gets it nor leaves it for later.
  assert.equal(keptChanges("project:p1", "u2", isText, noon, store), null);
  assert.equal(store.items.size, 0);
});

Deno.test("kept work is dropped when it is too old, unreadable or the wrong shape", () => {
  const store = memoryStore();
  const noon = Date.UTC(2026, 10, 25, 12);
  keepChanges("old", { userId: "u1", base: "", value: "x" }, noon, store);
  assert.equal(keptChanges("old", "u1", isText, noon + KEPT_FOR_MS + 1, store), null);
  // Kept "in the future" means the clock moved; do not trust it.
  keepChanges("future", { userId: "u1", base: "", value: "x" }, noon + 60_000, store);
  assert.equal(keptChanges("future", "u1", isText, noon, store), null);
  store.setItem("sl:kept-changes:broken", "{not json");
  assert.equal(keptChanges("broken", "u1", isText, noon, store), null);
  keepChanges("shape", { userId: "u1", base: "", value: 42 }, noon, store);
  assert.equal(keptChanges("shape", "u1", isText, noon, store), null);
  store.setItem(
    "sl:kept-changes:fields",
    JSON.stringify({ userId: "u1", value: "x", keptAt: noon }),
  );
  assert.equal(keptChanges("fields", "u1", isText, noon, store), null);
  assert.equal(store.items.size, 0);

  keepChanges("gone", { userId: "u1", base: "", value: "x" }, noon, store);
  forgetChanges("gone", store);
  assert.equal(keptChanges("gone", "u1", isText, noon, store), null);
});

Deno.test("blocked or full storage never stops the editor", () => {
  const failing: KeptStore = {
    getItem: () => {
      throw new DOMException("blocked", "SecurityError");
    },
    setItem: () => {
      throw new DOMException("full", "QuotaExceededError");
    },
    removeItem: () => {
      throw new DOMException("blocked", "SecurityError");
    },
  };
  keepChanges("k", { userId: "u1", base: "", value: "x" }, Date.now(), failing);
  assert.equal(keptChanges("k", "u1", isText, Date.now(), failing), null);
  forgetChanges("k", failing);
  keepChanges("k", { userId: "u1", base: "", value: "x" }, Date.now(), null);
  assert.equal(keptChanges("k", "u1", isText, Date.now(), null), null);
});

Deno.test("kept work goes back on its own only when nothing changed underneath it", () => {
  const kept = { userId: "u1", base: "7", value: { summary: "Edited" }, keptAt: 0 };
  assert.equal(restorePlan(null, { base: "7", value: { summary: "Saved" } }), "none");
  // Already saved after all: nothing to put back.
  assert.equal(restorePlan(kept, { base: "8", value: { summary: "Edited" } }), "none");
  assert.equal(restorePlan(kept, { base: "7", value: { summary: "Saved" } }), "restore");
  // Someone saved since: putting it back would replace their work, so the person chooses.
  assert.equal(restorePlan(kept, { base: "8", value: { summary: "Theirs" } }), "ask");
});

Deno.test("signing out clears every piece of kept work in the tab", () => {
  const store = listStore();
  const noon = Date.UTC(2026, 10, 25, 12);
  keepChanges("project:p1", { userId: "u1", base: "v1", value: "Mine" }, noon, store);
  keepChanges("committee-page:tech", { userId: "u2", base: "3", value: "Theirs" }, noon, store);
  store.setItem("sl:kept-changes:broken", "{not json");
  store.setItem("sl:ticker-paused", "1");
  clearKeptChanges(null, store);
  assert.deepEqual([...store.items.keys()], ["sl:ticker-paused"]);
});

Deno.test("when a session ends or someone signs in, only that person's kept work stays", () => {
  const store = listStore();
  const noon = Date.UTC(2026, 10, 25, 12);
  keepChanges("project:p1", { userId: "u1", base: "v1", value: "First" }, noon, store);
  keepChanges("announcement:new", { userId: "u2", base: "", value: "Second" }, noon, store);
  keepChanges("committee-page:tech", { userId: "u1", base: "3", value: "Third" }, noon, store);
  store.setItem("sl:kept-changes:broken", "{not json");
  store.setItem("sl:kept-changes:nobody", JSON.stringify({ value: "x" }));
  store.setItem("other", "left alone");

  clearKeptChanges("u1", store);
  assert.deepEqual(
    [...store.items.keys()],
    ["sl:kept-changes:project:p1", "sl:kept-changes:committee-page:tech", "other"],
  );
  // Then u1 signs out: nothing of theirs waits for the next person.
  clearKeptChanges(null, store);
  assert.deepEqual([...store.items.keys()], ["other"]);
});

Deno.test("clearing kept work never fails when storage is blocked or missing", () => {
  const blocked: KeptListStore = {
    ...memoryStore(),
    key: () => {
      throw new DOMException("blocked", "SecurityError");
    },
    get length(): number {
      throw new DOMException("blocked", "SecurityError");
    },
  };
  clearKeptChanges("u1", blocked);
  clearKeptChanges(null, null);
});

Deno.test("sign-out posts the ID token to Keycloak instead of putting it in an address", () => {
  const form = endSessionForm({
    endSessionUrl:
      "https://idp.scottylabs.org/realms/scottylabs/protocol/openid-connect/logout?id_token_hint=eyJ.a.b&client_id=scottylabs-org#x",
  });
  assert.deepEqual(form, {
    action: "https://idp.scottylabs.org/realms/scottylabs/protocol/openid-connect/logout",
    fields: [
      ["id_token_hint", "eyJ.a.b"],
      ["client_id", "scottylabs-org"],
    ],
  });
  assert.equal(endSessionForm({ endSessionUrl: "javascript:alert(1)" }), null);
  assert.equal(endSessionForm({}), null);
});

Deno.test("sign-out follows Keycloak's end-session address only when it is a safe one", () => {
  const url =
    "https://idp.scottylabs.org/realms/scottylabs/protocol/openid-connect/logout?id_token_hint=t&client_id=c";
  assert.equal(endSessionUrl({ endSessionUrl: url }), url);
  assert.equal(
    endSessionUrl({ endSessionUrl: "http://localhost:8180/realms/x/logout" }),
    "http://localhost:8180/realms/x/logout",
  );
  // The usual empty answer, and anything that is not a web address to leave for.
  for (const answer of [
    undefined,
    null,
    "",
    {},
    { endSessionUrl: null },
    { endSessionUrl: 42 },
    { endSessionUrl: "/portal/" },
    { endSessionUrl: "javascript:alert(1)" },
    { endSessionUrl: "http://idp.scottylabs.org/logout" },
    { endSessionUrl: "https://user:pass@idp.scottylabs.org/logout" },
    { endSessionUrl: "not a url" },
  ]) {
    assert.equal(endSessionUrl(answer), null, JSON.stringify(answer));
  }
});
