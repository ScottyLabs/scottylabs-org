// The four kinds of announcement, with the icon each shows on the ticker.

import Calendar from "@lucide/svelte/icons/calendar";
import Megaphone from "@lucide/svelte/icons/megaphone";
import Rocket from "@lucide/svelte/icons/rocket";
import Trophy from "@lucide/svelte/icons/trophy";
import type { AnnouncementKind } from "../api/types.ts";
import type { IconComponent } from "../portal/ui/types.ts";

export interface KindOption {
  value: AnnouncementKind;
  label: string;
  icon: IconComponent;
}

export const KINDS: readonly KindOption[] = [
  { value: "event", label: "Event", icon: Calendar },
  { value: "project_update", label: "Project update", icon: Rocket },
  { value: "milestone", label: "Milestone", icon: Trophy },
  { value: "club_news", label: "Club news", icon: Megaphone },
];

export function kindIcon(kind: AnnouncementKind): IconComponent {
  return KINDS.find((option) => option.value === kind)?.icon ?? Megaphone;
}

export function kindLabel(kind: AnnouncementKind): string {
  return KINDS.find((option) => option.value === kind)?.label ?? "Announcement";
}
