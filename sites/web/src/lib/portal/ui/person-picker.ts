// Helpers for PersonPicker: what a chosen person looks like, Andrew ID rules that match normalize_andrew_id in services/site/src/auth/mod.rs, and which options can be chosen.

import type { PeopleSearchResponse, PersonRef } from "../../api/types.ts";

/** Someone from the people search, or anyone else typed in by Andrew ID. */
export type PickedPerson = (PersonRef & { hasSignedIn?: boolean }) | { andrewId: string };

/** One person from GET /api/people/search. */
export type FoundPerson = PeopleSearchResponse["people"][number];

/** A row in the picker's list: a person the search found, or adding someone by Andrew ID. */
export type PickerOption =
  | { kind: "person"; key: string; person: FoundPerson }
  | { kind: "andrew_id"; key: string; andrewId: string };

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

/** True when the results on screen answer the text as it is now, not an earlier search. */
export function resultsAreCurrent(query: string, searchedFor: string, loading: boolean): boolean {
  return !loading && searchedFor === query.trim();
}

/**
 * The rows to show: the people found, less the excluded ones, then "Add <andrewid> by Andrew ID"
 * when the text is an Andrew ID nobody in the results has. That row waits for the search to
 * answer, so nobody adds a person twice.
 */
export function pickerOptions(
  found: readonly FoundPerson[],
  typedAndrewId: string | null,
  current: boolean,
  exclude: readonly string[],
): PickerOption[] {
  const options: PickerOption[] = found
    .filter((person) => !isExcluded(person, exclude))
    .map((person) => ({ kind: "person", key: `person-${person.id}`, person }));
  if (current && typedAndrewId !== null && !isExcluded({ andrewId: typedAndrewId }, exclude)) {
    const known = found.some((person) => person.andrewId.toLowerCase() === typedAndrewId);
    if (!known)
      options.push({ kind: "andrew_id", key: `andrew-${typedAndrewId}`, andrewId: typedAndrewId });
  }
  return options;
}

/**
 * The option a key press or click may choose. Results left over from an earlier search are
 * never chosen, so typing fast and pressing Enter cannot pick someone for the wrong text.
 */
export function choosableOption(
  options: readonly PickerOption[],
  index: number,
  current: boolean,
): PickerOption | undefined {
  return current ? options[index] : undefined;
}
