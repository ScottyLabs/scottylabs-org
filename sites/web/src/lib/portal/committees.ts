// Names, colors and tints for the seven committees and ScottyLabs as a whole ("club").
// Colors match COMMITTEE_COLORS in services/site/src/seed/mod.rs and the design file variables.

import { COMMITTEE_SLUGS, type CommitteeSlug, type TickerSource } from "../api/types.ts";

export interface CommitteeStyle {
  slug: TickerSource;
  name: string;
  /** Dot and accent color. */
  color: string;
  /** Light background for chips and covers. Text on it is #1e1e1e. */
  tint: string;
}

export const COMMITTEES: Readonly<Record<TickerSource, CommitteeStyle>> = {
  events: { slug: "events", name: "Events", color: "#B1B3E9", tint: "#DEDFF6" },
  outreach: { slug: "outreach", name: "Outreach", color: "#B1CAE9", tint: "#DEE9F6" },
  tech: { slug: "tech", name: "Tech", color: "#EC9798", tint: "#F7D3D4" },
  labrador: { slug: "labrador", name: "Labrador", color: "#E8BBAE", tint: "#F5E2DD" },
  design: { slug: "design", name: "Design", color: "#CDB3D2", tint: "#EADFEC" },
  finance: { slug: "finance", name: "Finance", color: "#B3D5E0", tint: "#DFEDF2" },
  foundry: { slug: "foundry", name: "Foundry", color: "#D3C7AE", tint: "#EDE7DD" },
  club: { slug: "club", name: "ScottyLabs", color: "#1E1E1E", tint: "#E6E6E6" },
};

/** The seven committees in the site's order. */
export const COMMITTEE_LIST: readonly CommitteeStyle[] = COMMITTEE_SLUGS.map(
  (slug) => COMMITTEES[slug],
);

/** Every ticker source: ScottyLabs first, then the committees. */
export const TICKER_SOURCE_LIST: readonly CommitteeStyle[] = [COMMITTEES.club, ...COMMITTEE_LIST];

export function isCommitteeSlug(value: unknown): value is CommitteeSlug {
  return typeof value === "string" && (COMMITTEE_SLUGS as readonly string[]).includes(value);
}

export function isTickerSource(value: unknown): value is TickerSource {
  return value === "club" || isCommitteeSlug(value);
}

export function committeeStyle(slug: TickerSource): CommitteeStyle {
  return COMMITTEES[slug];
}

export function committeeName(slug: TickerSource): string {
  return COMMITTEES[slug].name;
}

export function committeeColor(slug: TickerSource): string {
  return COMMITTEES[slug].color;
}

export function committeeTint(slug: TickerSource): string {
  return COMMITTEES[slug].tint;
}

/** "Events", "Events and Tech", "Events, Tech and Design". */
export function committeeNames(slugs: readonly TickerSource[]): string {
  const names = slugs.map((slug) => committeeName(slug));
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1) ?? ""}`;
}
