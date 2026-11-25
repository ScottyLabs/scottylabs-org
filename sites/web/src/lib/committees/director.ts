// The committee's director as the Team page shows them: name, role and photo, from src/lib/data/team.ts for the current year.

import type { CommitteeSlug } from "../api/types.ts";
import { YEARS } from "../data/team.ts";
import { directorRoleFor, personById, photoUrlFor } from "../sections/team/lookups.ts";

export interface Director {
  name: string;
  role: string;
  photoUrl: string | null;
  initials: string;
}

function initialsOf(name: string): string {
  const words = name.split(/\s+/u).filter((word) => word !== "");
  const first = words[0]?.[0] ?? "";
  const last = words.length > 1 ? (words.at(-1)?.[0] ?? "") : "";
  return `${first}${last}`.toUpperCase();
}

export function directorFor(slug: CommitteeSlug): Director | null {
  const id = YEARS[0]?.directors[slug];
  if (id === undefined) return null;
  const person = personById(id);
  return {
    name: person.name,
    role: directorRoleFor(slug),
    photoUrl: photoUrlFor(id) ?? null,
    initials: initialsOf(person.name),
  };
}
