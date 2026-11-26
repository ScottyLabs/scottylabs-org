// Tests for the analytics rules in src/lib/site/analytics.ts. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import {
  type AnalyticsEvent,
  PORTAL_ROOT,
  analyticsOptions,
  analyticsSettings,
  asksNotToBeTracked,
  isPortalPath,
  opensInFull,
  scrubEvent,
} from "../src/lib/site/analytics.ts";

const SETUP = {
  key: "phc_test",
  host: "https://v.scottylabs.org",
  pathname: "/",
  doNotTrack: false,
};

/** An event as PostHog hands it to before_send, with the fields scrubbing must leave alone. */
interface CapturedEvent extends AnalyticsEvent {
  uuid: string;
  event: string;
}

function event(
  properties: Record<string, unknown>,
  extra: Partial<CapturedEvent> = {},
): CapturedEvent {
  return { uuid: "u1", event: "$pageview", properties, ...extra };
}

Deno.test("portal paths are /portal and everything under it", () => {
  for (const path of ["/portal", "/portal/", "/portal/people/", "/portal/sign-in/"]) {
    assert.ok(isPortalPath(path), path);
  }
  for (const path of ["/", "/portals/", "/projects/portal/", "/portalx", ""]) {
    assert.ok(!isPortalPath(path), path);
  }
});

Deno.test("analytics load only on public pages, with both values, for visitors who allow it", () => {
  assert.deepEqual(analyticsSettings(SETUP), { key: "phc_test", host: "https://v.scottylabs.org" });
  assert.deepEqual(analyticsSettings({ ...SETUP, pathname: "/projects/nova/" }), {
    key: "phc_test",
    host: "https://v.scottylabs.org",
  });
  for (const pathname of ["/portal", "/portal/", "/portal/people/", "/portal/sign-in/"]) {
    assert.equal(analyticsSettings({ ...SETUP, pathname }), null, pathname);
  }
  assert.equal(analyticsSettings({ ...SETUP, key: undefined }), null);
  assert.equal(analyticsSettings({ ...SETUP, key: "  " }), null);
  assert.equal(analyticsSettings({ ...SETUP, host: undefined }), null);
  assert.equal(analyticsSettings({ ...SETUP, host: "" }), null);
  assert.equal(analyticsSettings({ ...SETUP, doNotTrack: true }), null);
});

Deno.test("Do Not Track and Global Privacy Control both count as asking not to be tracked", () => {
  assert.ok(asksNotToBeTracked({ doNotTrack: "1" }));
  assert.ok(asksNotToBeTracked({ doNotTrack: "yes" }));
  assert.ok(asksNotToBeTracked({ globalPrivacyControl: true }));
  assert.ok(!asksNotToBeTracked({}));
  assert.ok(!asksNotToBeTracked({ doNotTrack: "0", globalPrivacyControl: false }));
  assert.ok(!asksNotToBeTracked({ doNotTrack: null }));
  assert.ok(!asksNotToBeTracked({ doNotTrack: "unspecified" }));
});

Deno.test("PostHog sends page views and nothing else, and takes no settings or scripts from its host", () => {
  const options = analyticsOptions("https://v.scottylabs.org");
  assert.equal(options.api_host, "https://v.scottylabs.org");
  assert.equal(options.capture_pageview, "history_change");
  assert.equal(options.capture_pageleave, true);
  for (const off of [
    "autocapture",
    "rageclick",
    "capture_dead_clicks",
    "capture_heatmaps",
    "capture_exceptions",
    "capture_performance",
  ] as const) {
    assert.equal(options[off], false, off);
  }
  for (const on of [
    "disable_session_recording",
    "disable_surveys",
    "disable_product_tours",
    "disable_conversations",
    "disable_web_experiments",
    "advanced_disable_flags",
    "disable_external_dependency_loading",
    "mask_all_text",
    "mask_all_element_attributes",
    "respect_dnt",
  ] as const) {
    assert.equal(options[on], true, on);
  }
  assert.equal(options.persistence, "localStorage");
  assert.equal(options.cross_subdomain_cookie, false);
  assert.equal(typeof options.before_send, "function");
});

Deno.test("events from portal addresses never leave", () => {
  assert.equal(
    scrubEvent(event({ $current_url: "https://scottylabs.org/portal/people/" }), "/"),
    null,
  );
  assert.equal(scrubEvent(event({ $current_url: "https://scottylabs.org/portal" }), "/"), null);
  assert.equal(
    scrubEvent(
      event({ $current_url: "https://scottylabs.org/portal/projects/01a2/?review=9" }),
      "/",
    ),
    null,
  );
  // The browser already moved into the portal, whatever the event says.
  assert.equal(
    scrubEvent(event({ $current_url: "https://scottylabs.org/" }), "/portal/ticker/"),
    null,
  );
  assert.equal(scrubEvent(null, "/"), null);
});

Deno.test("public events keep their own addresses and lose portal ones", () => {
  const before = event(
    {
      $current_url: "https://scottylabs.org/projects/?q=nova",
      $pathname: "/projects/",
      $referrer: "https://scottylabs.org/portal/projects/01a214d9/?review=5",
      $initial_referrer: "https://scottylabs.org/portal/sign-in/?returnTo=%2Fportal%2Fpeople%2F",
      $prev_pageview_pathname: "/portal/people/",
      $session_entry_url: "https://scottylabs.org/committees/tech/",
      $referring_domain: "scottylabs.org",
      $screen_height: 900,
      title: "Projects",
    },
    {
      $set: { $referrer: "https://scottylabs.org/portal/" },
      $set_once: { $initial_pathname: "/portal/ticker/new/", $initial_os: "Mac OS X" },
    },
  );
  const after = scrubEvent(before, "/projects/");
  assert.ok(after !== null);
  assert.deepEqual(after.properties, {
    $current_url: "https://scottylabs.org/projects/?q=nova",
    $pathname: "/projects/",
    $referrer: `https://scottylabs.org${PORTAL_ROOT}`,
    $initial_referrer: `https://scottylabs.org${PORTAL_ROOT}`,
    $prev_pageview_pathname: PORTAL_ROOT,
    $session_entry_url: "https://scottylabs.org/committees/tech/",
    $referring_domain: "scottylabs.org",
    $screen_height: 900,
    title: "Projects",
  });
  assert.deepEqual(after.$set, { $referrer: `https://scottylabs.org${PORTAL_ROOT}` });
  assert.deepEqual(after.$set_once, { $initial_pathname: PORTAL_ROOT, $initial_os: "Mac OS X" });
  assert.equal(after.uuid, "u1");
  assert.equal(after.event, "$pageview");
  // The original is left alone.
  assert.equal(before.properties.$prev_pageview_pathname, "/portal/people/");
  // Text that only mentions the portal is not an address.
  const text = scrubEvent(
    event({ $current_url: "https://scottylabs.org/", note: "portal/people" }),
    "/",
  );
  assert.equal(text?.properties.note, "portal/people");
});

Deno.test("with analytics loaded, moving into the portal loads the page in full", () => {
  const home = new URL("https://scottylabs.org/projects/");
  const portal = new URL("https://scottylabs.org/portal/projects/");
  assert.ok(opensInFull(true, home, portal));
  assert.ok(opensInFull(true, home, new URL("https://scottylabs.org/portal")));
  // Without PostHog in the page, the router may take it.
  assert.ok(!opensInFull(false, home, portal));
  // Within the portal, or out of it, the router takes it.
  assert.ok(!opensInFull(true, portal, new URL("https://scottylabs.org/portal/people/")));
  assert.ok(!opensInFull(true, portal, home));
  assert.ok(!opensInFull(true, home, new URL("https://scottylabs.org/committees/")));
  assert.ok(!opensInFull(true, home, new URL("https://other.example/portal/")));
});
