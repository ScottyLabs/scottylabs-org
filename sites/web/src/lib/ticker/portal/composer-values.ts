// What the composer holds while someone writes, and the check for values kept in this tab when the session ended (lib/portal/kept-changes.ts). Plain logic, so it can be tested without a browser.

import type { AnnouncementKind, TickerSource } from "../../api/types.ts";
import { isTickerSource } from "../../portal/committees.ts";
import { isRecord } from "../../portal/kept-changes.ts";

export interface ComposerValues {
  kind: AnnouncementKind;
  committee: TickerSource;
  message: string;
  /** As typed; empty for no link. */
  linkUrl: string;
  /** RFC 3339, or null while no expiry is chosen. */
  expiresAt: string | null;
}

const KIND_VALUES: readonly string[] = [
  "event",
  "project_update",
  "milestone",
  "club_news",
] satisfies AnnouncementKind[];

/** Keys for kept composer values: a new one (posted again from `from`, if given) or one being edited. */
export function composerKey(editingId: string | null, from: string | null = null): string {
  if (editingId !== null) return `announcement:${editingId}`;
  return from === null ? "announcement:new" : `announcement:new:${from}`;
}

export function isComposerValues(value: unknown): value is ComposerValues {
  return (
    isRecord(value) &&
    typeof value.kind === "string" &&
    KIND_VALUES.includes(value.kind) &&
    isTickerSource(value.committee) &&
    typeof value.message === "string" &&
    typeof value.linkUrl === "string" &&
    (value.expiresAt === null || typeof value.expiresAt === "string")
  );
}
