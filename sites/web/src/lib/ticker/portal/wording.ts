// What the composer's main button and the note under it say, for each person and announcement.

import { listNames } from "../format.ts";

/**
 * - `edit-live`: an admin or director changing a live announcement.
 * - `review`: an admin or director changing someone's announcement while it waits for approval.
 * - `publish`: posting straight to the ticker.
 * - `submit`: a lead sending theirs for approval, the first time or again.
 */
export type ComposerRole = "edit-live" | "review" | "publish" | "submit";

export interface RoleInput {
  isLive: boolean;
  isWaiting: boolean;
  /** The viewer manages the announcement's source, so they approve it. */
  manages: boolean;
  /** Posting from the chosen source goes live without approval. */
  postsDirectly: boolean;
}

export function composerRole({
  isLive,
  isWaiting,
  manages,
  postsDirectly,
}: RoleInput): ComposerRole {
  if (isLive) return "edit-live";
  if (isWaiting) return manages ? "review" : "submit";
  return postsDirectly ? "publish" : "submit";
}

export function submitLabel(role: ComposerRole): string {
  if (role === "publish") return "Publish announcement";
  if (role === "submit") return "Send for approval";
  return "Save changes";
}

/**
 * The line under the button: for leads, who approves; for approvers, what saving does. Null when
 * the button says it all.
 */
export function actionNote(
  role: ComposerRole,
  committee: string,
  approverNames: readonly string[],
): string | null {
  if (role === "review") {
    return "Saving keeps it waiting for approval. To put it on the ticker, approve it under Needs attention on the overview.";
  }
  if (role !== "submit") return null;
  if (approverNames.length === 0) {
    return `An admin approves announcements from leads in ${committee} before they go live.`;
  }
  const who =
    approverNames.length === 1
      ? `Director of ${committee}, approves`
      : `directors of ${committee}, approve`;
  return `${listNames(approverNames)}, ${who} announcements from leads before they go live.`;
}
