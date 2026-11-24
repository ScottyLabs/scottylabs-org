// Calls to the ticker endpoints (docs/implementation/contract.md, "Ticker"), typed with the shapes in types.ts.

import { del, get, patch, post, put } from "../../api/client.ts";
import type {
  Announcement,
  AnnouncementInput,
  AnnouncementListResponse,
  AnnouncementResponse,
  AnnouncementTab,
  Permissions,
  PersonRef,
  TickerSource,
} from "../../api/types.ts";

export function listAnnouncements(
  tab: AnnouncementTab,
  signal?: AbortSignal,
): Promise<AnnouncementListResponse> {
  return get<AnnouncementListResponse>("/announcements", { query: { tab }, signal });
}

export async function getAnnouncement(id: string, signal?: AbortSignal): Promise<Announcement> {
  return (await get<AnnouncementResponse>(`/announcements/${encodeURIComponent(id)}`, { signal }))
    .announcement;
}

export async function createAnnouncement(input: AnnouncementInput): Promise<Announcement> {
  return (await post<AnnouncementResponse>("/announcements", input)).announcement;
}

export async function updateAnnouncement(
  id: string,
  changes: Partial<AnnouncementInput>,
): Promise<Announcement> {
  return (await patch<AnnouncementResponse>(`/announcements/${encodeURIComponent(id)}`, changes))
    .announcement;
}

export async function extendAnnouncement(id: string, days: number): Promise<Announcement> {
  return (
    await post<AnnouncementResponse>(`/announcements/${encodeURIComponent(id)}/extend`, { days })
  ).announcement;
}

export async function endAnnouncement(id: string): Promise<Announcement> {
  return (await post<AnnouncementResponse>(`/announcements/${encodeURIComponent(id)}/end`))
    .announcement;
}

export function deleteAnnouncement(id: string): Promise<void> {
  return del(`/announcements/${encodeURIComponent(id)}`);
}

export function reorderAnnouncements(ids: readonly string[]): Promise<AnnouncementListResponse> {
  return put<AnnouncementListResponse>("/announcements/order", { ids });
}

/** Whether posting from this source goes live at once, as `permissions::ticker_posting` decides. */
export function postsDirectly(permissions: Permissions, source: TickerSource): boolean {
  return permissions.isAdmin || (source !== "club" && permissions.directorOf.includes(source));
}

export function canReorder(permissions: Permissions): boolean {
  return permissions.isAdmin || permissions.directorOf.length > 0;
}

/** The directors who approve a lead's announcements from a committee. */
export function approversOf(
  approvers: AnnouncementListResponse["approvers"],
  committee: TickerSource,
): PersonRef[] {
  return approvers.find((group) => group.committee === committee)?.people ?? [];
}
