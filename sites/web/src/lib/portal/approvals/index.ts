// Reviewing what leads submit, and showing leads where their submissions stand.
//   import { ApprovalDialog, listApprovals } from "#lib/portal/approvals/index.ts";
// /portal/?review=<approvalId> opens the review dialog on the overview.

export { default as AnnouncementPreview } from "./AnnouncementPreview.svelte";
export { default as ApprovalDialog } from "./ApprovalDialog.svelte";
export { approve, listApprovals, sendBack } from "./api.ts";
export {
  ANNOUNCEMENT_KIND_LABELS,
  MAX_NOTE_LENGTH,
  approvedMessage,
  linkHost,
  projectCardFromDoc,
  reviewDescription,
  reviewTitle,
  sentBackMessage,
  submissionStatus,
  submissionSubject,
  submitterFirstName,
  submitterName,
  type SubmissionStatus,
} from "./format.ts";

/** The overview address that opens the review dialog for one submission. */
export function reviewHref(approvalId: string): string {
  return `/portal/?review=${encodeURIComponent(approvalId)}`;
}
