// Helpers for PersonPicker: what a chosen person looks like, and Andrew ID rules that match normalize_andrew_id in services/site/src/auth/mod.rs.

import type { PersonRef } from "../../api/types.ts";

/** Someone from the people search, or anyone else typed in by Andrew ID. */
export type PickedPerson = (PersonRef & { hasSignedIn?: boolean }) | { andrewId: string };

export function isKnownPerson(
  person: PickedPerson,
): person is PersonRef & { hasSignedIn?: boolean } {
  return "id" in person;
}

/** The Andrew ID in lower case, or null when it cannot be one. */
export function normalizeAndrewId(raw: string): string | null {
  const id = raw.trim().toLowerCase();
  return /^[a-z0-9][a-z0-9._-]{0,63}$/u.test(id) ? id : null;
}

export function personName(person: PickedPerson): string {
  return isKnownPerson(person) && person.name.trim() !== "" ? person.name : person.andrewId;
}

export function personInitials(person: PickedPerson): string {
  if (isKnownPerson(person) && person.initials.trim() !== "") return person.initials;
  return person.andrewId.slice(0, 2);
}

/** True when `person` is one of `exclude`, given as user ids or Andrew IDs. */
export function isExcluded(person: PickedPerson, exclude: readonly string[]): boolean {
  if (exclude.length === 0) return false;
  const andrewId = person.andrewId.toLowerCase();
  return exclude.some(
    (entry) => entry.toLowerCase() === andrewId || (isKnownPerson(person) && entry === person.id),
  );
}
