// Helpers for the portal's projects pages: what to call a project's state, and drafts shaped as the cards and pages visitors would see, for previews.

import type {
  ProjectAdminSummary,
  ProjectCard,
  ProjectDetail,
  ProjectDoc,
  ProjectState,
} from "../api/types.ts";
import { plainText } from "../editor/sanitize.ts";

/** Longest card excerpt; matches EXCERPT_CHARS in services/site/src/routes/projects/doc.rs. */
export const EXCERPT_CHARS = 240;
export const MAX_NAME_CHARS = 60;
export const MAX_SUMMARY_CHARS = 48;
export const MAX_DESCRIPTION_CHARS = 4000;
export const MAX_PHOTOS = 8;

export interface StateLabel {
  label: string;
  /** A StatusDot tone. */
  tone: "success" | "warning" | "neutral";
}

/** How a project's state reads in the portal list and editor. */
export function stateLabel(project: {
  state: ProjectState;
  hasUnpublishedChanges: boolean;
}): StateLabel {
  if (project.state === "archived") return { label: "Archived", tone: "neutral" };
  if (project.state === "draft") return { label: "Draft", tone: "neutral" };
  if (project.hasUnpublishedChanges) return { label: "Unpublished changes", tone: "warning" };
  return { label: "Published", tone: "success" };
}

/** The Drafts, Published and Archived filter in the portal list. */
export type StateFilter = ProjectState | "all";

export function isStateFilter(value: unknown): value is StateFilter {
  return value === "all" || value === "draft" || value === "published" || value === "archived";
}

/** Rows matching the search text by name or address. */
export function matchesRow(row: ProjectAdminSummary, q: string): boolean {
  const needle = q.trim().toLowerCase();
  if (needle === "") return true;
  return row.name.toLowerCase().includes(needle) || row.slug.includes(needle);
}

/** The first `max` characters of the description's text, cut at a word, like the server. */
export function excerpt(html: string, max = EXCERPT_CHARS): string {
  const text = plainText(html).replaceAll(/\s+/gu, " ").trim();
  const chars = Array.from(text);
  if (chars.length <= max) return text;
  const cut = chars.slice(0, max).join("");
  const space = cut.lastIndexOf(" ");
  const trimmed = (space > 0 ? cut.slice(0, space) : cut).replace(/[,.;:]+$/u, "");
  return `${trimmed}\u2026`;
}

interface ProjectMeta {
  id: string;
  slug: string;
  featured: boolean;
}

/** A draft as the card visitors would see once it is published. */
export function draftCard(doc: ProjectDoc, meta: ProjectMeta): ProjectCard {
  const trimmedName = doc.name.trim();
  return {
    id: meta.id,
    slug: meta.slug,
    name: trimmedName === "" ? "Untitled project" : trimmedName,
    summary: doc.summary.trim(),
    excerpt: excerpt(doc.description),
    committee: doc.committee,
    status: doc.status,
    kind: doc.kind,
    cover: doc.media[0] ?? null,
    siteUrl: blankToNull(doc.siteUrl),
    repoUrl: blankToNull(doc.repoUrl),
    featured: meta.featured,
  };
}

/** A draft as the page visitors would see once it is published. */
export function draftDetail(
  doc: ProjectDoc,
  meta: ProjectMeta,
  publishedAt = new Date().toISOString(),
): ProjectDetail {
  return {
    ...draftCard(doc, meta),
    description: doc.description,
    builtWith: doc.builtWith.filter((committee) => committee !== doc.committee),
    media: doc.media,
    videoUrl: blankToNull(doc.videoUrl),
    publishedAt,
  };
}

function blankToNull(value: string | null): string | null {
  return value === null || value.trim() === "" ? null : value.trim();
}

/** Two drafts with the same content, for knowing whether there is anything to save. */
export function sameDoc(a: ProjectDoc, b: ProjectDoc): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
