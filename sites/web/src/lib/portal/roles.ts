// Plain-language descriptions of a person's roles, for the account menu and page headers.

import type { MeResponse, Permissions, RoleAssignment } from "../api/types.ts";
import { committeeNames } from "./committees.ts";

function committeesFor(roles: readonly RoleAssignment[], role: RoleAssignment["role"]) {
  return roles.flatMap((r) => (r.role === role && r.committee !== null ? [r.committee] : []));
}

/**
 * One line naming every role: "Admin, exec board", "Director of Events",
 * "Director of Events, lead in Tech". Empty for people with no role.
 */
export function describeRoles(roles: readonly RoleAssignment[]): string {
  const parts: string[] = [];
  if (roles.some((r) => r.role === "admin")) parts.push("admin, exec board");
  const directs = committeesFor(roles, "director");
  if (directs.length > 0) parts.push(`director of ${committeeNames(directs)}`);
  const leads = committeesFor(roles, "lead");
  if (leads.length > 0) parts.push(`lead in ${committeeNames(leads)}`);
  const line = parts.join(", ");
  return line.charAt(0).toUpperCase() + line.slice(1);
}

/** The highest role as a short label for a chip: "Admin", "Director of Events", "Lead in Tech". */
export function primaryRoleLabel(roles: readonly RoleAssignment[]): string | null {
  if (roles.some((r) => r.role === "admin")) return "Admin";
  const directs = committeesFor(roles, "director");
  if (directs.length > 0) return `Director of ${committeeNames(directs)}`;
  const leads = committeesFor(roles, "lead");
  if (leads.length > 0) return `Lead in ${committeeNames(leads)}`;
  return null;
}

/** How this session signed in, as shown in the account menu. */
export function describeAuthMode(mode: MeResponse["authMode"]): string {
  return mode === "oidc"
    ? "Signed in with ScottyLabs single sign-on"
    : "Signed in with development sign-in";
}

/** Whether the People section is open to this person. */
export function canSeePeople(permissions: Permissions): boolean {
  return permissions.peopleScope === "all" || permissions.peopleScope.length > 0;
}

/** "Thomas" from "Thomas Kanz"; the Andrew ID when the name is empty. */
export function firstName(user: { name: string; andrewId: string }): string {
  const first = user.name.trim().split(/\s+/u)[0];
  return first === undefined || first === "" ? user.andrewId : first;
}
