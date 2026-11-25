// Where committee pages send people. The same links the home page and the footer use.

import type { CommitteeSlug } from "../api/types.ts";

/** The club sign-up on TartanConnect, from the home page. */
export const JOIN_URL = "https://tartanconnect.cmu.edu/scottylabs/club_signup";

/** The ScottyLabs Slack invite, from the site footer. Channels are found once inside. */
export const SLACK_URL = "https://go.scottylabs.org/slack";

export function committeePath(slug: CommitteeSlug): string {
  return `/committees/${slug}/`;
}

/** The Projects page filtered to one committee. */
export function committeeProjectsPath(slug: CommitteeSlug): string {
  return `/projects/?committee=${slug}`;
}

/** True for links that leave the site, which open in a new tab. */
export function isExternal(url: string): boolean {
  return url.startsWith("https://") || url.startsWith("http://");
}

/** "scottylabs.org/committees/tech", as the publish dialog says it. */
export function displayPath(slug: CommitteeSlug): string {
  return `scottylabs.org/committees/${slug}`;
}
