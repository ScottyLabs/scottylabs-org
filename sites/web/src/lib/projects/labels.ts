// Words for project facts, shared by the public pages, the portal and the tests.

import type { ProjectKind, ProjectStatus } from "../api/types.ts";

export const PROJECT_STATUS_LABELS: Readonly<Record<ProjectStatus, string>> = {
  live: "Live",
  in_development: "In development",
  annual: "Annual",
  ongoing: "Ongoing",
};

export const PROJECT_KIND_LABELS: Readonly<Record<ProjectKind, string>> = {
  app: "App",
  event: "Event",
  tool: "Tool",
  brand: "Brand",
  program: "Program",
  fundraiser: "Fundraiser",
  other: "Other",
};

/** The public page of a project, with the trailing slash the site serves it under. */
export function projectPath(slug: string): string {
  return `/projects/${encodeURIComponent(slug)}/`;
}

/** The host of a link for a short label, such as "tartanhacks.com" for https://www.tartanhacks.com/. */
export function linkHost(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./u, "");
  } catch {
    return url;
  }
}

/** A repository link without the scheme, such as "git.cmu.dev/ScottyLabs/maps". */
export function linkLabel(url: string): string {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname.replace(/\/+$/u, "");
    return `${parsed.host.replace(/^www\./u, "")}${path}`;
  } catch {
    return url;
  }
}
