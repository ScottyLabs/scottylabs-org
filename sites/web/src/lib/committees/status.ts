// How the portal names a page's state.

import type { CommitteePageStatus } from "../api/types.ts";
import { formatUpdated } from "../portal/dates.ts";
import type { StatusTone } from "../portal/ui/types.ts";

export const STATUS_LABELS: Readonly<Record<CommitteePageStatus, string>> = {
  published: "Published",
  unpublished_changes: "Unpublished changes",
  never_published: "Not published yet",
};

export const STATUS_TONES: Readonly<Record<CommitteePageStatus, StatusTone>> = {
  published: "success",
  unpublished_changes: "warning",
  never_published: "neutral",
};

/** The editor's badge: Published while nothing waits, Draft otherwise. */
export function editorBadge(status: CommitteePageStatus): "Published" | "Draft" {
  return status === "published" ? "Published" : "Draft";
}

/** "Saved just now", "Saved 3 min ago". The service's clock may run a little ahead of the browser's. */
export function savedLabel(savedAt: string, now: Date): string {
  const reference = new Date(Math.max(now.getTime(), Date.parse(savedAt)));
  return `Saved ${formatUpdated(savedAt, reference).toLowerCase()}`;
}
