// Calls to the committee page endpoints. See "Committee pages" in docs/implementation/contract.md.

import { del, get, post, put } from "../api/client.ts";
import type {
  CommitteePageDoc,
  CommitteePageEditorResponse,
  CommitteePagesResponse,
  CommitteeSlug,
  EditLock,
  PageChangesResponse,
  PublicCommitteePageResponse,
  PublicCommitteesResponse,
  SaveDraftResponse,
} from "../api/types.ts";

const pagePath = (slug: CommitteeSlug) => `/committee-pages/${slug}`;

export function loadPublicCommittees(signal?: AbortSignal): Promise<PublicCommitteesResponse> {
  return get<PublicCommitteesResponse>("/public/committees", {
    signal,
    redirectOnUnauthenticated: false,
  });
}

export function loadPublicPage(
  slug: CommitteeSlug,
  signal?: AbortSignal,
): Promise<PublicCommitteePageResponse> {
  return get<PublicCommitteePageResponse>(`/public/committees/${slug}`, {
    signal,
    redirectOnUnauthenticated: false,
  });
}

export function loadPages(signal?: AbortSignal): Promise<CommitteePagesResponse> {
  return get<CommitteePagesResponse>("/committee-pages", { signal });
}

export function loadEditor(
  slug: CommitteeSlug,
  signal?: AbortSignal,
): Promise<CommitteePageEditorResponse> {
  return get<CommitteePageEditorResponse>(pagePath(slug), { signal });
}

/** Takes the five minute edit lock, or refreshes it. Fails with 409 while someone else holds it. */
export function takeLock(slug: CommitteeSlug): Promise<EditLock> {
  return post<EditLock>(`${pagePath(slug)}/lock`);
}

/** Admins only: takes the lock from whoever holds it, for an urgent fix. The service records it. */
export function takeOverLock(slug: CommitteeSlug): Promise<EditLock> {
  return post<EditLock>(`${pagePath(slug)}/lock/take-over`);
}

export function releaseLock(slug: CommitteeSlug): Promise<void> {
  return del(`${pagePath(slug)}/lock`);
}

/**
 * Lets go of the lock while the page unloads. A keepalive request finishes after the tab closes,
 * which the API client's fetch would not.
 */
export function releaseLockOnExit(slug: CommitteeSlug, csrfToken: string | null): void {
  if (csrfToken === null) return;
  void fetch(`/api${pagePath(slug)}/lock`, {
    method: "DELETE",
    headers: { "X-CSRF-Token": csrfToken },
    credentials: "same-origin",
    keepalive: true,
  }).catch(() => {
    // The lock expires by itself after five minutes.
  });
}

export function saveDraft(
  slug: CommitteeSlug,
  doc: CommitteePageDoc,
  baseVersion: number,
): Promise<SaveDraftResponse> {
  return put<SaveDraftResponse>(`${pagePath(slug)}/draft`, { doc, baseVersion });
}

export function loadChanges(
  slug: CommitteeSlug,
  signal?: AbortSignal,
): Promise<PageChangesResponse> {
  return get<PageChangesResponse>(`${pagePath(slug)}/changes`, { signal });
}

export function publishPage(
  slug: CommitteeSlug,
  draftVersion: number,
): Promise<CommitteePageEditorResponse> {
  return post<CommitteePageEditorResponse>(`${pagePath(slug)}/publish`, { draftVersion });
}

export function discardChanges(slug: CommitteeSlug): Promise<CommitteePageEditorResponse> {
  return post<CommitteePageEditorResponse>(`${pagePath(slug)}/discard`);
}
