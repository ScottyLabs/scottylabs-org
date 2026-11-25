// Choosing a project's editors: only leads of the committee that owns the project can be one.
// Pure functions, so the component and the tests share them.

import type { CommitteeSlug, PersonRef, PersonRow } from "../../api/types.ts";

/** The people with a lead role in `committee`, from GET /api/people?committee=. */
export function leadsIn(people: readonly PersonRow[], committee: CommitteeSlug): PersonRef[] {
  return people
    .filter((row) => row.roles.some((role) => role.role === "lead" && role.committee === committee))
    .map(({ person }) => ({
      id: person.id,
      andrewId: person.andrewId,
      name: person.name,
      initials: person.initials,
    }));
}

/** The leads who are not editors yet, by name. */
export function leadChoices(
  leads: readonly PersonRef[],
  editors: readonly PersonRef[],
): PersonRef[] {
  const taken = new Set(editors.map((editor) => editor.id));
  return leads
    .filter((lead) => !taken.has(lead.id))
    .toSorted((a, b) => leadLabel(a).localeCompare(leadLabel(b)));
}

/** "Sara Chokshi (spchoksh)", or the Andrew ID alone when that is all we know. */
export function leadLabel(person: PersonRef): string {
  const name = person.name.trim();
  return name === "" || name === person.andrewId ? person.andrewId : `${name} (${person.andrewId})`;
}
