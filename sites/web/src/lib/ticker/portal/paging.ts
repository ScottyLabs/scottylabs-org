// The expired tab comes 50 at a time, newest first (GET /api/announcements?tab=expired&before=).
// "Show more" adds the next page under the rows already shown.

import type { Announcement, AnnouncementListResponse } from "../../api/types.ts";

/**
 * The list with the next page added at the end. Rows already shown are not repeated, in case one
 * moved between pages; the counts and the cursor come from the newer answer.
 */
export function appendPage(
  shown: AnnouncementListResponse,
  next: AnnouncementListResponse,
): { list: AnnouncementListResponse; added: Announcement[] } {
  const known = new Set(shown.announcements.map((announcement) => announcement.id));
  const added = next.announcements.filter((announcement) => !known.has(announcement.id));
  return { list: { ...next, announcements: [...shown.announcements, ...added] }, added };
}

/** What screen readers hear after "Show more": "Added 50 more. Showing 100 of 120." */
export function addedMessage(added: number, shown: number, total: number): string {
  const what = added === 1 ? "1 more announcement" : `${added.toString()} more announcements`;
  const all = Math.max(total, shown);
  const showing =
    shown >= all
      ? `Showing all ${all.toString()}.`
      : `Showing ${shown.toString()} of ${all.toString()}.`;
  return added === 0 ? `No more announcements. ${showing}` : `Added ${what}. ${showing}`;
}
