// Filtering the Projects page by committee and search text, and keeping both in the address.
// Pure functions, so the page and the tests share them.

import type { CommitteeSlug, ProjectCard } from "../api/types.ts";
import { COMMITTEES, isCommitteeSlug } from "../portal/committees.ts";
import { PROJECT_KIND_LABELS, PROJECT_STATUS_LABELS } from "./labels.ts";

export interface ProjectFilters {
  /** null for All. */
  committee: CommitteeSlug | null;
  /** The search text as typed. */
  q: string;
}

export const NO_FILTERS: ProjectFilters = { committee: null, q: "" };

/** The longest search text the page keeps. */
export const MAX_QUERY_LENGTH = 80;

/** Reads `?committee=` and `?q=`. Unknown committees mean All. */
export function filtersFromParams(params: URLSearchParams): ProjectFilters {
  const committee = params.get("committee");
  return {
    committee: isCommitteeSlug(committee) ? committee : null,
    q: (params.get("q") ?? "").slice(0, MAX_QUERY_LENGTH),
  };
}

/** "?committee=tech&q=maps", or "" without filters. Empty search text is left out. */
export function filtersQuery(filters: ProjectFilters): string {
  const params = new URLSearchParams();
  if (filters.committee !== null) params.set("committee", filters.committee);
  const q = filters.q.trim();
  if (q !== "") params.set("q", q);
  const query = params.toString();
  return query === "" ? "" : `?${query}`;
}

/**
 * What the search box should hold after a navigation that set `fromAddress`. The address keeps the
 * search trimmed, so text being typed ("cmu ") stays as it is while it says the same thing.
 */
export function searchTextAfterNavigation(typed: string, fromAddress: string): string {
  return typed.trim() === fromAddress.trim() ? typed : fromAddress;
}

/** Lowercase and without accents, so "cafe" finds "Caf\u00E9". */
export function fold(text: string): string {
  return text
    .normalize("NFKD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** Everything a search looks through: name, summary, excerpt, committee, status and kind. */
export function searchableText(project: ProjectCard): string {
  return fold(
    [
      project.name,
      project.summary,
      project.excerpt,
      COMMITTEES[project.committee].name,
      PROJECT_KIND_LABELS[project.kind],
      PROJECT_STATUS_LABELS[project.status],
    ].join(" "),
  );
}

/** True when every word of the search text appears somewhere in the project. */
export function matchesSearch(project: ProjectCard, q: string): boolean {
  const words = fold(q)
    .split(/\s+/u)
    .filter((word) => word !== "");
  if (words.length === 0) return true;
  const text = searchableText(project);
  return words.every((word) => text.includes(word));
}

export function filterProjects(
  projects: readonly ProjectCard[],
  filters: ProjectFilters,
): ProjectCard[] {
  return projects.filter(
    (project) =>
      (filters.committee === null || project.committee === filters.committee) &&
      matchesSearch(project, filters.q),
  );
}

export type CommitteeCounts = Record<CommitteeSlug | "all", number>;

/** How many projects each committee tab would show for the search text. */
export function committeeCounts(projects: readonly ProjectCard[], q: string): CommitteeCounts {
  const counts: CommitteeCounts = {
    all: 0,
    events: 0,
    outreach: 0,
    tech: 0,
    labrador: 0,
    design: 0,
    finance: 0,
    foundry: 0,
  };
  for (const project of projects) {
    if (!matchesSearch(project, q)) continue;
    counts.all += 1;
    counts[project.committee] += 1;
  }
  return counts;
}

/** "38 projects", "14 projects in Tech", "4 projects match \u201Chack\u201D". */
export function resultsLine(count: number, filters: ProjectFilters): string {
  const noun = count === 1 ? "project" : "projects";
  const where = filters.committee === null ? "" : ` in ${COMMITTEES[filters.committee].name}`;
  const q = filters.q.trim();
  if (q === "") return `${count.toString()} ${noun}${where}`;
  const verb = count === 1 ? "matches" : "match";
  return `${count.toString()} ${noun}${where} ${verb} \u201C${q}\u201D`;
}

/**
 * Projects to show with this one: others from the same committee first, then the rest in the
 * order given (featured first, then by name).
 */
export function relatedProjects(
  projects: readonly ProjectCard[],
  current: { id: string; committee: CommitteeSlug },
  limit = 4,
): ProjectCard[] {
  const others = projects.filter((project) => project.id !== current.id);
  const same = others.filter((project) => project.committee === current.committee);
  const rest = others.filter((project) => project.committee !== current.committee);
  return [...same, ...rest].slice(0, limit);
}

// The Projects page remembers its filters for the visit, so the breadcrumb on a project page can lead back to the same list (spec P-11). Storage can be off or full; then the link is plain.
const RETURN_KEY = "sl:projects:filters";

export function rememberFilters(filters: ProjectFilters): void {
  try {
    sessionStorage.setItem(RETURN_KEY, filtersQuery(filters));
  } catch {
    // Not remembering is fine.
  }
}

/** The Projects page address with the filters last used in this tab. */
export function projectsIndexHref(): string {
  let query = "";
  try {
    query = sessionStorage.getItem(RETURN_KEY) ?? "";
  } catch {
    query = "";
  }
  // Only ever a query string made by filtersQuery, never a different address.
  const safe = query.startsWith("?")
    ? filtersQuery(filtersFromParams(new URLSearchParams(query)))
    : "";
  return `/projects/${safe}`;
}
