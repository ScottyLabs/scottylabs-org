// Analytics for the public site (contract, Changes, "security audit fixes: client-side security and privacy"). PostHog counts page views on public pages and nothing else: no clicks, element text or form fields, no recordings or surveys, no settings or scripts from the analytics host, and nothing from the portal.
//
// - PostHog loads only when the page load starts on a public page, both public values are set, and the visitor has not asked not to be tracked (Do Not Track or Global Privacy Control). A page load that starts in the portal never loads it, even after the person moves on to public pages.
// - Once it is loaded, moving into the portal is a full page load (see `opensInFull`, used by the root layout), so the portal never runs with PostHog in the page.
// - As a second layer, every event is checked before it leaves (`scrubEvent`): one sent from a portal address is dropped, and portal addresses in the others, such as the referrer, are cut to /portal/.
//
// Plain logic, so it can be tested without a browser. It imports nothing from posthog-js, so the tests need no npm packages; the root layout passes these options to posthog.init, which checks them against the library's types.

type Properties = Record<string, unknown>;

/** The parts of a PostHog event (posthog-js's CaptureResult) that leave the browser with it. */
export interface AnalyticsEvent {
  properties: Properties;
  $set?: Properties;
  $set_once?: Properties;
}

/** What portal addresses in event properties are cut to. */
export const PORTAL_ROOT = "/portal/";

/** True for /portal and every address under it. */
export function isPortalPath(pathname: string): boolean {
  return pathname === "/portal" || pathname.startsWith("/portal/");
}

/** The browser's "do not track me" signals, as `navigator` has them. */
export interface TrackingPreferences {
  doNotTrack?: string | null;
  globalPrivacyControl?: boolean;
}

/** True when the visitor asked not to be tracked, through Do Not Track or Global Privacy Control. */
export function asksNotToBeTracked(preferences: TrackingPreferences): boolean {
  return (
    preferences.doNotTrack === "1" ||
    preferences.doNotTrack === "yes" ||
    preferences.globalPrivacyControl === true
  );
}

export interface AnalyticsSetup {
  key: string | undefined;
  host: string | undefined;
  /** The path the page load started on. */
  pathname: string;
  doNotTrack: boolean;
}

/** The key and host to load PostHog with, or null when it must not load. */
export function analyticsSettings(setup: AnalyticsSetup): { key: string; host: string } | null {
  const key = setup.key?.trim();
  const host = setup.host?.trim();
  if (key === undefined || key === "" || host === undefined || host === "") return null;
  if (isPortalPath(setup.pathname) || setup.doNotTrack) return null;
  return { key, host };
}

/**
 * The options PostHog starts with. Each feature that could send more than a page view is off here,
 * and nothing the analytics host sends can turn one on: remote settings and feature flags are not
 * fetched, and no script is loaded from it, which the site's security policy would refuse anyway.
 */
export function analyticsOptions(host: string) {
  return {
    api_host: host,
    ui_host: "https://us.posthog.com",
    defaults: "2025-05-24",
    // Page views, and the page leave events that time them.
    capture_pageview: "history_change",
    capture_pageleave: true,
    autocapture: false,
    rageclick: false,
    capture_dead_clicks: false,
    capture_heatmaps: false,
    capture_exceptions: false,
    capture_performance: false,
    disable_session_recording: true,
    disable_surveys: true,
    disable_product_tours: true,
    disable_conversations: true,
    disable_web_experiments: true,
    advanced_disable_flags: true,
    disable_external_dependency_loading: true,
    // Anything that still looks at an element sees no text and no attributes.
    mask_all_text: true,
    mask_all_element_attributes: true,
    respect_dnt: true,
    // No cookie: the anonymous id stays in this browser instead of travelling with every request,
    // the portal's included, and other scottylabs.org sites cannot read it.
    persistence: "localStorage",
    cross_subdomain_cookie: false,
    before_send: <Event extends AnalyticsEvent>(event: Event | null) => scrubEvent(event),
  } as const;
}

/** A portal address cut to the portal's root, or null when `value` is not a portal address. */
function cutPortalAddress(value: string): string | null {
  if (value.startsWith("/")) {
    const [path = ""] = value.split(/[?#]/u, 1);
    return isPortalPath(path) ? PORTAL_ROOT : null;
  }
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return null;
  return isPortalPath(url.pathname) ? `${url.origin}${PORTAL_ROOT}` : null;
}

function scrubValue(value: unknown): unknown {
  return typeof value === "string" ? (cutPortalAddress(value) ?? value) : value;
}

function scrubProperties(properties: Properties): Properties {
  return Object.fromEntries(
    Object.entries(properties).map(([name, value]) => [name, scrubValue(value)]),
  );
}

/** The path the browser is on, or null outside a browser. */
function currentPath(): string | null {
  try {
    return location.pathname;
  } catch {
    return null;
  }
}

/**
 * The event as it may leave the browser: null for one sent from a portal address, either the one
 * it records or the one the browser is on, and otherwise the event with portal addresses in its
 * properties cut to /portal/.
 */
export function scrubEvent<Event extends AnalyticsEvent>(
  event: Event | null,
  pathname: string | null = currentPath(),
): Event | null {
  if (event === null) return null;
  if (pathname !== null && isPortalPath(pathname)) return null;
  const url = event.properties.$current_url;
  if (typeof url === "string" && cutPortalAddress(url) !== null) return null;
  const scrubbed: Event = { ...event, properties: scrubProperties(event.properties) };
  if (event.$set !== undefined) scrubbed.$set = scrubProperties(event.$set);
  if (event.$set_once !== undefined) scrubbed.$set_once = scrubProperties(event.$set_once);
  return scrubbed;
}

/**
 * Whether a navigation from `from` to `to` has to load the page in full instead of in the router:
 * with analytics loaded, every move into the portal does, so the portal never runs with PostHog.
 */
export function opensInFull(analyticsLoaded: boolean, from: URL, to: URL): boolean {
  return (
    analyticsLoaded &&
    to.origin === from.origin &&
    isPortalPath(to.pathname) &&
    !isPortalPath(from.pathname)
  );
}
