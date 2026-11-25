// What the overview says, worked out from GET /api/overview and the person's permissions.
// Kept free of Svelte so the wording can be tested.

import type {
  ActivityEntry,
  CommitteeSlug,
  ImageRef,
  OverviewResponse,
  Permissions,
} from "../../api/types.ts";
import { committeeName, committeeNames } from "../committees.ts";
import { formatRelative, formatShortDate, formatTime, partOfDay } from "../dates.ts";

/** Where to add a project. The projects track serves the form there. */
export const NEW_PROJECT_HREF = "/portal/projects/new/";

export interface CardAction {
  label: string;
  href: string;
}

export interface CardCopy {
  title: string;
  /** Small text at the top right, such as "5 of 6 slots". */
  meta: string;
  headline: string;
  text: string;
  primary: CardAction | null;
  secondary: CardAction | null;
}

function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** "is" or "are" for a count. */
function verb(count: number): string {
  return count === 1 ? "is" : "are";
}

/** A message without its closing punctuation, so it reads inside a sentence. */
function inSentence(message: string): string {
  return message.trim().replace(/[.!?]+$/u, "");
}

/** "Good afternoon, Thomas." */
export function greeting(firstName: string, now: Date = new Date()): string {
  return `Good ${partOfDay(now)}, ${firstName}.`;
}

/** The line under the greeting. */
export function overviewDescription(permissions: Permissions): string {
  if (permissions.isAdmin) return "Here is what is live on scottylabs.org and what needs you.";
  if (permissions.directorOf.length > 0) {
    return `You can edit ${committeeNames(permissions.directorOf)}. Here is what is live and what needs you.`;
  }
  return "Here is your work on scottylabs.org and what needs you.";
}

// ---------------------------------------------------------------------------
// Ticker

export interface TickerCopy extends CardCopy {
  /** Why "New announcement" is turned off, or null when it is not. */
  blocked: string | null;
}

export function tickerCopy(
  ticker: OverviewResponse["ticker"],
  permissions: Permissions,
  now: Date = new Date(),
): TickerCopy {
  const full = ticker.live >= ticker.slots;
  const canPost = permissions.tickerSources.length > 0;
  // Leads may still send one for approval; it waits until a slot is free.
  const blocked =
    canPost && full && !permissions.tickerNeedsApproval
      ? `All ${ticker.slots} slots are in use. End one to post another.`
      : null;

  let headline: string;
  if (permissions.isAdmin) {
    headline =
      ticker.live === 0 ? "Nothing is live" : `${plural(ticker.live, "announcement")} live`;
  } else if (ticker.mineLive === 0) {
    headline = "None of yours is live";
  } else {
    headline = `${ticker.mineLive} of yours ${verb(ticker.mineLive)} live`;
  }

  let text: string;
  const next = ticker.nextToExpire;
  if (next !== null) {
    const when = formatRelative(next.expiresAt, now);
    text = permissions.isAdmin
      ? `Next to leave: ${inSentence(next.message)}, ${when}.`
      : `${inSentence(next.message)} leaves ${when}.`;
  } else if (canPost) {
    text = "Post an announcement to show it under the home page hero.";
  } else {
    text = "Ask your director if you need to post to the ticker.";
  }

  return {
    title: "Ticker",
    meta: `${ticker.live} of ${ticker.slots} slots`,
    headline,
    text,
    primary: canPost ? { label: "New announcement", href: "/portal/ticker/new/" } : null,
    secondary: { label: "Manage", href: "/portal/ticker/" },
    blocked,
  };
}

// ---------------------------------------------------------------------------
// Committee pages

export interface CommitteesCopy extends CardCopy {
  covers: { slug: CommitteeSlug; cover: ImageRef | null }[];
  /** One page in focus, shown with a wider cover. */
  single: boolean;
}

export function committeesCopy(
  committees: OverviewResponse["committees"],
  permissions: Permissions,
): CommitteesCopy {
  const pagesMeta = plural(committees.total, "page");
  const allPublished =
    committees.published === committees.total
      ? `${pagesMeta} published`
      : `${committees.published} of ${pagesMeta} published`;
  const editsOne = !permissions.isAdmin && committees.editable.length === 1;
  const ownSlug = committees.editable[0];

  if (editsOne && ownSlug !== undefined) {
    const name = committeeName(ownSlug);
    const changed = committees.withUnpublishedChanges.includes(ownSlug);
    return {
      title: "Your committee page",
      meta: `/committees/${ownSlug}`,
      headline: changed ? `${name} page has unpublished changes` : `${name} page is published`,
      text: changed
        ? "Publish them when they are ready. Visitors see the last published version."
        : "Visitors see the latest version.",
      primary: { label: `Edit the ${name} page`, href: `/portal/committees/${ownSlug}/` },
      secondary: null,
      covers: committees.covers.filter((entry) => entry.slug === ownSlug),
      single: true,
    };
  }

  const edits = committees.editable.length > 0;
  const changed = committees.withUnpublishedChanges;
  let text: string;
  if (!edits) text = "Directors edit their committee pages. You can look at any of them.";
  else if (changed.length === 0) text = "Every page you edit is up to date.";
  else {
    text = `${committeeNames(changed)} ${changed.length === 1 ? "has" : "have"} changes that are not published yet.`;
  }
  const covers =
    permissions.isAdmin || !edits
      ? committees.covers
      : committees.covers.filter((entry) => committees.editable.includes(entry.slug));
  return {
    title: edits && !permissions.isAdmin ? "Your committee pages" : "Committee pages",
    meta: pagesMeta,
    headline: allPublished,
    text,
    primary: { label: "Open committee pages", href: "/portal/committees/" },
    secondary: null,
    covers,
    single: false,
  };
}

// ---------------------------------------------------------------------------
// Projects

export function projectsCopy(
  projects: OverviewResponse["projects"],
  permissions: Permissions,
): CardCopy {
  const canCreate = permissions.projectCommittees.length > 0;
  const manage = { label: "Manage", href: "/portal/projects/" };
  const add = { label: "Add project", href: NEW_PROJECT_HREF };

  if (permissions.isAdmin) {
    return {
      title: "Projects",
      meta: plural(projects.total, "project"),
      headline: `${projects.published} published, ${plural(projects.drafts, "draft")}`,
      text:
        projects.drafts === 0
          ? "Every project is on the site."
          : `${plural(projects.drafts, "draft")} ${verb(projects.drafts)} not on the site yet.`,
      primary: add,
      secondary: manage,
    };
  }

  if (canCreate) {
    const owner = committeeNames(permissions.projectCommittees);
    let text: string;
    if (projects.inScope === 0) text = "Add your committee\u2019s first project.";
    else if (projects.drafts === 0) {
      text =
        projects.inScope === 1
          ? "It is published."
          : projects.inScope === 2
            ? "Both are published."
            : "All of them are published.";
    } else {
      text = projects.drafts === 1 ? "1 is still a draft." : `${projects.drafts} are still drafts.`;
    }
    return {
      title: "Projects",
      meta: plural(projects.inScope, "project"),
      headline: `${projects.inScope} ${owner} ${projects.inScope === 1 ? "project" : "projects"}`,
      text,
      primary: add,
      secondary: manage,
    };
  }

  return {
    title: "Your projects",
    meta: plural(projects.inScope, "project"),
    headline:
      projects.inScope === 0 ? "No projects yet" : `${plural(projects.inScope, "project")} to edit`,
    text:
      projects.inScope === 0
        ? "Your director can add you to a project."
        : "A director approves what you publish.",
    primary:
      projects.inScope === 0 ? null : { label: "Open your projects", href: "/portal/projects/" },
    secondary: null,
  };
}

// ---------------------------------------------------------------------------
// Recent activity

/** "Evan Foster published the Events page", or "You published ..." for the viewer. */
export function activityText(entry: ActivityEntry, viewerId: string): string {
  if (entry.actor === null) return entry.summary;
  const who = entry.actor.id === viewerId ? "You" : entry.actor.name;
  return `${who} ${lowerFirst(entry.summary)}`;
}

/** Lowercases the first letter of a sentence unless the first word is an acronym or a name such as "CMU". */
function lowerFirst(text: string): string {
  const [first = "", second = ""] = text;
  if (first === "" || second.toUpperCase() === second) return text;
  return first.toLowerCase() + text.slice(1);
}

/** "11:42 AM" today, "Yesterday", then "Nov 20". */
export function activityTime(at: string, now: Date = new Date()): string {
  const day = formatShortDate(at, now);
  if (day === formatShortDate(now, now)) return formatTime(at);
  const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000);
  if (day === formatShortDate(yesterday, now)) return "Yesterday";
  return day;
}
