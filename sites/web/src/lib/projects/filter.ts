// Filtering the Projects page by committee and search text, and keeping both in the address.
// Pure functions, so the page and the tests share them.
//
// The page filters the cards it has loaded as you type. Cards carry only the start of each description, so searches of two or more characters also go to the service (GET /api/public/projects?q=), which matches the same way against the whole description, and the projects it finds are added to the ones found here.

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

/** The folded words of a search. The service splits and folds them the same way. */
export function searchWords(q: string): string[] {
  return fold(q)
    .split(/\s+/u)
    .filter((word) => word !== "");
}

/** True when every word of the search text appears somewhere in the project. */
export function matchesSearch(project: ProjectCard, q: string): boolean {
  const words = searchWords(q);
  if (words.length === 0) return true;
  const text = searchableText(project);
  return words.every((word) => text.includes(word));
}

const NONE: ReadonlySet<string> = new Set();

/**
 * The projects shown for the filters: those in the committee that match the search here, plus
 * those the service matched (`alsoMatching`, by id; see remoteMatches), in the order given.
 */
export function filterProjects(
  projects: readonly ProjectCard[],
  filters: ProjectFilters,
  alsoMatching: ReadonlySet<string> = NONE,
): ProjectCard[] {
  return projects.filter(
    (project) =>
      (filters.committee === null || project.committee === filters.committee) &&
      (matchesSearch(project, filters.q) || alsoMatching.has(project.id)),
  );
}

/** Searches at least this long also go to the service, which reads whole descriptions. */
export const MIN_REMOTE_QUERY = 2;

/**
 * Two searches with the same key find the same projects: the folded words, one space apart.
 * "Caf\u00E9 " and "cafe" share one.
 */
export function searchKey(q: string): string {
  return searchWords(q).join(" ");
}

/** The search to send to the service, trimmed, or null when the loaded cards are enough. */
export function remoteQuery(q: string): string | null {
  const trimmed = q.trim();
  if (Array.from(trimmed).length < MIN_REMOTE_QUERY || searchKey(trimmed) === "") return null;
  return trimmed;
}

/** What the service found for one search key. */
export interface RemoteAnswer {
  key: string;
  ids: ReadonlySet<string>;
}

/**
 * Where the service's search for one key stands. While it is pending, `carried` holds an earlier
 * answer this search narrows (see pendingSearch).
 */
export type RemoteSearch =
  | { key: string; status: "pending"; carried?: RemoteAnswer }
  | { key: string; status: "done"; ids: ReadonlySet<string> }
  | { key: string; status: "failed" };

/**
 * True when every project that matches the later search also matches the earlier one: each
 * earlier word is inside a later word, as when typing on ("zeph" to "zephyrine tiles").
 */
export function narrows(earlier: string, later: string): boolean {
  const words = later.split(" ");
  return earlier.split(" ").every((word) => words.some((other) => other.includes(word)));
}

/**
 * The state while the service looks for `key`. While the search only narrows, the projects the
 * service alone found for the words typed so far (matches deep in a description) stay until it
 * answers, instead of vanishing at every key press and coming back; the new answer can only take
 * them away. Matches on the loaded cards are left out: the page narrows those itself, at once.
 */
export function pendingSearch(
  current: RemoteSearch | null,
  key: string,
  projects: readonly ProjectCard[],
): RemoteSearch {
  const known = lastAnswer(current);
  if (known === undefined || !narrows(known.key, key)) return { key, status: "pending" };
  const deep = projects.filter(
    (project) => known.ids.has(project.id) && !matchesSearch(project, known.key),
  );
  return {
    key,
    status: "pending",
    carried: { key: known.key, ids: new Set(deep.map((project) => project.id)) },
  };
}

/** The newest answer the service gave, if it still counts for this search or one before it. */
function lastAnswer(current: RemoteSearch | null): RemoteAnswer | undefined {
  if (current?.status === "done") return { key: current.key, ids: current.ids };
  return current?.status === "pending" ? current.carried : undefined;
}

/**
 * The ids the service matched for this search text, to add to the local matches: its answer for
 * this very search, or while that is pending an earlier answer it narrows. None when the request
 * failed: the local result stands.
 */
export function remoteMatches(remote: RemoteSearch | null, q: string): ReadonlySet<string> {
  if (remote === null || remoteQuery(q) === null || remote.key !== searchKey(q)) return NONE;
  if (remote.status === "done") return remote.ids;
  return remote.status === "pending" ? (remote.carried?.ids ?? NONE) : NONE;
}

/**
 * True while the service has yet to answer for this search text, so "no matches" may still
 * change. Searches too short to send never wait, and neither do ones whose request failed.
 */
export function awaitingRemote(remote: RemoteSearch | null, q: string): boolean {
  if (remoteQuery(q) === null) return false;
  return remote === null || remote.key !== searchKey(q) || remote.status === "pending";
}

export type CommitteeCounts = Record<CommitteeSlug | "all", number>;

/** How many projects each committee tab would show for the search text. */
export function committeeCounts(
  projects: readonly ProjectCard[],
  q: string,
  alsoMatching: ReadonlySet<string> = NONE,
): CommitteeCounts {
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
    if (!matchesSearch(project, q) && !alsoMatching.has(project.id)) continue;
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
