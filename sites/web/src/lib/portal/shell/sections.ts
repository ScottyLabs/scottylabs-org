// The portal's sections, as the top bar and the phone tab bar list them.

import LayoutGrid from "@lucide/svelte/icons/layout-grid";
import Lock from "@lucide/svelte/icons/lock";
import Megaphone from "@lucide/svelte/icons/megaphone";
import SquareKanban from "@lucide/svelte/icons/square-kanban";
import Users from "@lucide/svelte/icons/users";
import type { Permissions } from "../../api/types.ts";
import { canSeePeople } from "../roles.ts";
import type { IconComponent } from "../ui/types.ts";

export type SectionId = "overview" | "ticker" | "committees" | "projects" | "people";

export interface PortalSection {
  id: SectionId;
  label: string;
  /** The label in the phone tab bar, where space is short. */
  shortLabel: string;
  href: string;
  icon: IconComponent;
}

const ALL_SECTIONS: readonly PortalSection[] = [
  { id: "overview", label: "Overview", shortLabel: "Overview", href: "/portal/", icon: LayoutGrid },
  { id: "ticker", label: "Ticker", shortLabel: "Ticker", href: "/portal/ticker/", icon: Megaphone },
  {
    id: "committees",
    label: "Committees",
    shortLabel: "Pages",
    href: "/portal/committees/",
    icon: Users,
  },
  {
    id: "projects",
    label: "Projects",
    shortLabel: "Projects",
    href: "/portal/projects/",
    icon: SquareKanban,
  },
  { id: "people", label: "People", shortLabel: "People", href: "/portal/people/", icon: Lock },
];

/** Sections this person can open. People is hidden from leads (spec O-2). */
export function sectionsFor(permissions: Permissions | null): PortalSection[] {
  return ALL_SECTIONS.filter((section) => {
    if (section.id !== "people") return true;
    return permissions !== null && canSeePeople(permissions);
  });
}

/** The sections a person without a role sees, greyed out, on the no-access page. */
export const LOCKED_PREVIEW_SECTIONS: readonly PortalSection[] = ALL_SECTIONS.filter(
  (section) => section.id !== "people",
);

export function isSectionActive(section: PortalSection, pathname: string): boolean {
  const path = pathname.endsWith("/") ? pathname : `${pathname}/`;
  if (section.id === "overview") return path === section.href;
  return path.startsWith(section.href);
}
