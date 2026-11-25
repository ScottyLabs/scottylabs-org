// Calls to /api/approvals. See "Approvals" in docs/implementation/contract.md.

import { get, post } from "../../api/client.ts";
import type { Approval, ApprovalListResponse } from "../../api/types.ts";

/** Waiting submissions for the committees this person approves for, or their own with `mine`. */
export function listApprovals(
  status: "waiting" | "decided",
  options: { mine?: boolean; signal?: AbortSignal } = {},
): Promise<ApprovalListResponse> {
  return get<ApprovalListResponse>("/approvals", {
    query: { status, mine: options.mine === true ? true : undefined },
    signal: options.signal,
  });
}

/** Publishes what was submitted. A note is optional and goes to the lead. */
export function approve(id: string, note: string): Promise<Approval> {
  const trimmed = note.trim();
  return post<Approval>(
    `/approvals/${encodeURIComponent(id)}/approve`,
    trimmed === "" ? undefined : { note: trimmed },
  );
}

/** Returns it to the lead with a note that says what to change. */
export function sendBack(id: string, note: string): Promise<Approval> {
  return post<Approval>(`/approvals/${encodeURIComponent(id)}/send-back`, { note: note.trim() });
}
