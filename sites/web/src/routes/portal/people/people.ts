// The People page's rules and wording: one row per role, what each role can edit, who appointed it, and the confirmations. Kept free of Svelte so it can be tested.

import type {
  CommitteeSlug,
  GovernanceSyncStatus,
  PeopleRoleAssignment,
  PersonRow,
  ProjectAdminSummary,
  Role,
  RoleAssignment,
} from "../../../lib/api/types.ts";
import { COMMITTEE_SLUGS } from "../../../lib/api/types.ts";
import { committeeName } from "../../../lib/portal/committees.ts";
import { formatAbsolute, formatUpdated } from "../../../lib/portal/dates.ts";

/** One role of one person, as a table row. */
export interface AssignmentRow {
  person: PersonRow["person"];
  assignment: PeopleRoleAssignment;
  isMe: boolean;
}

/** Shown under a director or lead whom governance did not list at the last sync, so admins can review the role. */
export function governanceNote(assignment: PeopleRoleAssignment): string | null {
  if (assignment.governanceListed !== false) return null;
  return assignment.role === "director"
    ? "Not the director in governance"
    : "Not a head in governance";
}

/** The service syncs every six hours; a day without a successful sync means something is wrong. */
const STALE_SYNC_MS = 24 * 60 * 60 * 1000;

/** What admins should know about the last governance sync, or null when all is well. */
export function syncNotice(
  status: GovernanceSyncStatus | undefined,
  now: Date,
): { tone: "warning" | "info"; title: string; text: string } | null {
  if (status === undefined) return null;
  const last =
    status.lastSuccessAt === null
      ? "Directors and heads have not been synced from governance yet."
      : `Directors and heads were last synced from governance ${formatAbsolute(status.lastSuccessAt, now)}.`;
  if (status.lastError !== null) {
    return {
      tone: "warning",
      title: "The last governance sync did not work",
      text: `${status.lastError} ${last}`,
    };
  }
  if (status.lastSuccessAt === null) {
    return { tone: "info", title: "Not synced yet", text: last };
  }
  if (now.getTime() - new Date(status.lastSuccessAt).getTime() > STALE_SYNC_MS) {
    return {
      tone: "warning",
      title: "Governance roles may be out of date",
      text: `${last} The site syncs every six hours, so roles may not match governance. Sync now to check.`,
    };
  }
  return null;
}

export const ROLE_LABELS: Readonly<Record<Role, string>> = {
  admin: "Admin",
  director: "Director",
  lead: "Lead",
};

const ROLE_ORDER: Readonly<Record<Role, number>> = { admin: 0, director: 1, lead: 2 };

function committeeOrder(slug: CommitteeSlug | null): number {
  return slug === null ? -1 : COMMITTEE_SLUGS.indexOf(slug);
}

/**
 * Every role as its own row: admins, then directors and leads in committee order, then by name.
 * `committees` keeps only roles in those committees, for a director's team.
 */
export function assignmentRows(
  people: readonly PersonRow[],
  viewerId: string,
  committees: readonly CommitteeSlug[] | "all",
): AssignmentRow[] {
  const rows = people.flatMap((row) =>
    row.roles
      .filter(
        (assignment) =>
          committees === "all" ||
          (assignment.committee !== null && committees.includes(assignment.committee)),
      )
      .map((assignment) => ({ person: row.person, assignment, isMe: row.person.id === viewerId })),
  );
  return rows.toSorted(
    (a, b) =>
      ROLE_ORDER[a.assignment.role] - ROLE_ORDER[b.assignment.role] ||
      committeeOrder(a.assignment.committee) - committeeOrder(b.assignment.committee) ||
      a.person.name.localeCompare(b.person.name),
  );
}

/** "an admin", "director of Events", "a lead in Events". */
export function rolePhrase(role: Role, committee: CommitteeSlug | null): string {
  if (role === "admin") return "an admin";
  const name = committee === null ? "" : committeeName(committee);
  return role === "director" ? `director of ${name}` : `a lead in ${name}`;
}

/** The confirmation once someone has a role: "Helen Fu is now a lead for Events." */
export function nowHasRole(name: string, role: Role, committee: CommitteeSlug | null): string {
  if (role === "lead" && committee !== null)
    return `${name} is now a lead for ${committeeName(committee)}.`;
  return `${name} is now ${rolePhrase(role, committee)}.`;
}

export function removedRole(name: string, assignment: RoleAssignment): string {
  return `Removed ${name} as ${rolePhrase(assignment.role, assignment.committee)}.`;
}

/** Project names by id, from GET /api/projects. */
export type ProjectNames = ReadonlyMap<string, string>;

export function projectNames(projects: readonly ProjectAdminSummary[]): ProjectNames {
  return new Map(projects.map((project) => [project.id, project.name]));
}

/** "Everything", "Events page, Events projects, ticker", "Nova, TartanHacks, ticker". */
export function canEditText(assignment: RoleAssignment, names: ProjectNames | null): string {
  if (assignment.role === "admin") return "Everything";
  const committee = assignment.committee === null ? "" : committeeName(assignment.committee);
  if (assignment.role === "director") return `${committee} page, ${committee} projects, ticker`;
  const parts = leadProjects(assignment.projectIds, names);
  if (assignment.canPostTicker) parts.push("ticker");
  return parts.length === 0 ? "Nothing yet" : parts.join(", ");
}

/** A lead's projects by name, or how many there are while the names are unknown. */
function leadProjects(ids: readonly string[], names: ProjectNames | null): string[] {
  if (names === null) {
    if (ids.length === 0) return [];
    return [ids.length === 1 ? "1 project" : `${ids.length} projects`];
  }
  return ids
    .flatMap((id) => {
      const name = names.get(id);
      return name === undefined ? [] : [name];
    })
    .toSorted((a, b) => a.localeCompare(b));
}

const MINUTE_MS = 60 * 1000;

/** "just now", "12 min ago", then "Sep 14", for the middle of a sentence. */
function appointedWhen(at: string, now: Date): string {
  const age = now.getTime() - new Date(at).getTime();
  // A role added a moment ago carries the service's clock, which can run a little ahead.
  if (age > -MINUTE_MS && age < MINUTE_MS) return "just now";
  return formatUpdated(at, now);
}

/** "By you, just now", "By you, Sep 14", "By Evan Foster, Sep 14", "By an admin", "From governance". */
export function appointedText(
  assignment: RoleAssignment,
  viewerId: string,
  now: Date = new Date(),
): string {
  if (assignment.source === "governance") return "From governance";
  if (assignment.source === "keycloak_group") return "From the admin group";
  if (assignment.appointedBy === null) return "By an admin";
  const who = assignment.appointedBy.id === viewerId ? "you" : assignment.appointedBy.name;
  return `By ${who}, ${appointedWhen(assignment.appointedAt, now)}`;
}

/** Why a lead's projects go when they become a director or an admin. */
export function clearedProjectsNote(role: Exclude<Role, "lead">): string {
  return role === "admin"
    ? "Admins edit every project, so the list of projects is cleared."
    : "Directors edit every project of their committee, so the list of projects is cleared.";
}

/** The projects a lead in `committee` can be given: that committee's, not archived, by name. */
export function projectsFor(
  projects: readonly ProjectAdminSummary[],
  committee: CommitteeSlug | null,
): ProjectAdminSummary[] {
  if (committee === null) return [];
  return projects
    .filter((project) => project.committee === committee && project.state !== "archived")
    .toSorted((a, b) => a.name.localeCompare(b.name));
}

/** Andrew IDs of people who already have a role in a committee, so the picker leaves them out. */
export function peopleIn(rows: readonly AssignmentRow[], committee: CommitteeSlug): string[] {
  return rows
    .filter((row) => row.assignment.committee === committee)
    .map((row) => row.person.andrewId);
}

/** The assignment a save created or changed, found in the person's updated row. */
export function matchingAssignment(
  row: PersonRow,
  role: Role,
  committee: CommitteeSlug | null,
): RoleAssignment | undefined {
  return row.roles.find(
    (assignment) => assignment.role === role && assignment.committee === committee,
  );
}

/** Replaces one person's row in the list, or adds it. */
export function withPerson(people: readonly PersonRow[], updated: PersonRow): PersonRow[] {
  const exists = people.some((row) => row.person.id === updated.person.id);
  return exists
    ? people.map((row) => (row.person.id === updated.person.id ? updated : row))
    : [...people, updated];
}

/** Takes away one role; a person with none left drops out of the list. */
export function withoutAssignment(people: readonly PersonRow[], assignmentId: string): PersonRow[] {
  return people
    .map((row) => ({ ...row, roles: row.roles.filter((role) => role.id !== assignmentId) }))
    .filter((row) => row.roles.length > 0);
}
