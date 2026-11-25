// Words for approvals: what the review dialog, the toasts and a lead's list of submissions say.
// Kept free of Svelte so the rules can be tested.

import type { AnnouncementKind, Approval, ProjectCard, ProjectDoc } from "../../api/types.ts";
import { plainText } from "../../editor/sanitize.ts";
import { formatShortDate } from "../dates.ts";

export const ANNOUNCEMENT_KIND_LABELS: Readonly<Record<AnnouncementKind, string>> = {
  event: "Event",
  project_update: "Project update",
  milestone: "Milestone",
  club_news: "Club news",
};

/** Longest note an approver can leave, as the service allows. */
export const MAX_NOTE_LENGTH = 500;

/** Card excerpts stop around here, like the service's. */
const EXCERPT_LENGTH = 240;

/** "Sara Chokshi", or "A lead" when the submitter's account is gone. */
export function submitterName(approval: Pick<Approval, "submittedBy">): string {
  return approval.submittedBy?.name ?? "A lead";
}

/** "Sara", for addressing the submitter in the note label and toasts. */
export function submitterFirstName(approval: Pick<Approval, "submittedBy">): string {
  const first = approval.submittedBy?.name.trim().split(/\s+/u)[0];
  return first === undefined || first === "" ? "the lead" : first;
}

/** "Nova", or "the project" when the snapshot could not be read. */
function projectName(approval: Pick<Approval, "project">): string {
  const name = approval.project?.name.trim();
  return name === undefined || name === "" ? "the project" : name;
}

/**
 * The review dialog's title: "Sara Chokshi wants to post this", "Sara Chokshi wants to publish
 * Nova", or "Sara Chokshi asked to publish Nova" once it is decided.
 */
export function reviewTitle(approval: Approval): string {
  const who = submitterName(approval);
  const asks = approval.status === "waiting" ? "wants" : "asked";
  return approval.subjectType === "announcement"
    ? `${who} ${asks} to post this`
    : `${who} ${asks} to publish ${projectName(approval)}`;
}

/** The line under the title: how approval works, or what came of it once decided. */
export function reviewDescription(approval: Approval): string {
  const announcement = approval.subjectType === "announcement";
  const lead = submitterFirstName(approval);
  if (approval.status === "waiting") {
    return announcement
      ? "Posts from leads go live after a director approves them."
      : "Changes from leads go live after a director approves them.";
  }
  if (approval.status === "approved") {
    return announcement
      ? "It was approved and went on the ticker, so nothing is left to decide."
      : "The changes were approved and went live, so nothing is left to decide.";
  }
  if (approval.status === "sent_back") {
    return announcement
      ? `It was sent back to ${lead}, so nothing is left to decide.`
      : `The changes were sent back to ${lead}, so nothing is left to decide.`;
  }
  return `${capitalized(lead)} changed it after sending it, so nothing is left to decide.`;
}

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

/** The confirmation after "Approve and publish". */
export function approvedMessage(approval: Approval): string {
  return approval.subjectType === "announcement"
    ? "Approved. It is on the ticker now."
    : `Approved. ${projectName(approval)} is on the site now.`;
}

/** The confirmation after "Send back". */
export function sentBackMessage(approval: Approval): string {
  return `Sent back to ${submitterFirstName(approval)} with your note.`;
}

/** The submitted project as its card will look, for the preview. Drafts have no page yet. */
export function projectCardFromDoc(id: string, doc: ProjectDoc): ProjectCard {
  return {
    id,
    slug: "",
    name: doc.name,
    summary: doc.summary,
    excerpt: excerpt(plainText(doc.description)),
    committee: doc.committee,
    status: doc.status,
    kind: doc.kind,
    cover: doc.media[0] ?? null,
    siteUrl: doc.siteUrl,
    repoUrl: doc.repoUrl,
    featured: false,
  };
}

function excerpt(text: string): string {
  if (text.length <= EXCERPT_LENGTH) return text;
  const cut = text.slice(0, EXCERPT_LENGTH);
  const space = cut.lastIndexOf(" ");
  return `${(space > 0 ? cut.slice(0, space) : cut).replace(/[,.;:]+$/u, "")}\u2026`;
}

/** The status dot's color; one of the portal's StatusTone values. */
export type SubmissionTone = "success" | "warning" | "info" | "neutral";

/** A lead's submission as their own list shows it: what it is and where it stands. */
export interface SubmissionStatus {
  tone: SubmissionTone;
  label: string;
  /** Who decided and when, or how long it has waited. */
  detail: string;
}

export function submissionSubject(approval: Approval): string {
  if (approval.subjectType === "project") return `Changes to ${projectName(approval)}`;
  return approval.announcement?.message ?? "An announcement";
}

export function submissionStatus(approval: Approval, now: Date = new Date()): SubmissionStatus {
  if (approval.status === "waiting") {
    return {
      tone: "info",
      label: "Waiting",
      detail: `Sent ${formatShortDate(approval.submittedAt, now)}`,
    };
  }
  if (approval.status === "withdrawn") {
    return { tone: "neutral", label: "Withdrawn", detail: "You changed it after sending it" };
  }
  const decider = approval.decidedBy?.name ?? "A director";
  const when = approval.decidedAt === null ? "" : `, ${formatShortDate(approval.decidedAt, now)}`;
  return approval.status === "approved"
    ? { tone: "success", label: "Approved", detail: `By ${decider}${when}` }
    : { tone: "warning", label: "Sent back", detail: `By ${decider}${when}` };
}
