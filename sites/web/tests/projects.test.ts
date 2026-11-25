/// <reference lib="dom" />
// Tests for the projects helpers. Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import { ApiError } from "../src/lib/api/client.ts";
import type { PersonRef, PersonRow, ProjectCard, ProjectDoc } from "../src/lib/api/types.ts";
import {
  draftCard,
  draftDetail,
  excerpt,
  filterAnnouncement,
  isStateFilter,
  matchesRow,
  projectCount,
  sameDoc,
  stateLabel,
} from "../src/lib/projects/drafts.ts";
import {
  committeeCounts,
  filterProjects,
  filtersFromParams,
  filtersQuery,
  fold,
  matchesSearch,
  relatedProjects,
  resultsLine,
  searchTextAfterNavigation,
} from "../src/lib/projects/filter.ts";
import { leadChoices, leadLabel, leadsIn } from "../src/lib/projects/editor/editors.ts";
import { retriesOnItsOwn } from "../src/lib/projects/editor/retry.ts";
import { linkLabel, projectPath } from "../src/lib/projects/labels.ts";

function card(fields: Partial<ProjectCard>): ProjectCard {
  return {
    id: fields.slug ?? "id",
    slug: "project",
    name: "Project",
    summary: "",
    excerpt: "",
    committee: "tech",
    status: "live",
    kind: "app",
    cover: null,
    siteUrl: null,
    repoUrl: null,
    videoUrl: null,
    featured: false,
    ...fields,
  };
}

const PROJECTS = [
  card({
    slug: "tartanhacks",
    name: "TartanHacks",
    committee: "events",
    kind: "event",
    status: "annual",
    featured: true,
    excerpt: "Every February, students build.",
  }),
  card({ slug: "cmu-maps", name: "CMU Maps", summary: "Room-level navigation of campus." }),
  card({
    slug: "nova",
    name: "Nova",
    committee: "events",
    kind: "event",
    excerpt: "ScottyLabs\u2019 fall hackathon.",
  }),
  card({ slug: "cafe", name: "Caf\u00E9 finder", committee: "labrador" }),
];

Deno.test("filters come from the address and go back into it", () => {
  assert.deepEqual(filtersFromParams(new URLSearchParams("committee=tech&q=maps")), {
    committee: "tech",
    q: "maps",
  });
  assert.deepEqual(filtersFromParams(new URLSearchParams("committee=marketing")), {
    committee: null,
    q: "",
  });
  assert.equal(filtersFromParams(new URLSearchParams(`q=${"x".repeat(200)}`)).q.length, 80);
  assert.equal(filtersQuery({ committee: null, q: "  " }), "");
  assert.equal(filtersQuery({ committee: "tech", q: " hack day " }), "?committee=tech&q=hack+day");
});

Deno.test("typing a word, a space, a pause and another word keeps both words", () => {
  // "cmu " is typed, and after the pause the address says ?q=cmu.
  let typed = "cmu ";
  const settled = filtersQuery({ committee: null, q: typed });
  assert.equal(settled, "?q=cmu");
  const fromAddress = filtersFromParams(new URLSearchParams(settled)).q;
  typed = searchTextAfterNavigation(typed, fromAddress);
  assert.equal(typed, "cmu ");
  // Typing goes on from there.
  typed += "maps";
  assert.equal(filtersQuery({ committee: null, q: typed }), "?q=cmu+maps");
  assert.ok(matchesSearch(card({ name: "CMU Maps" }), typed));
  assert.equal(
    resultsLine(1, { committee: null, q: typed }),
    "1 project matches \u201Ccmu maps\u201D",
  );
  // A link to another search replaces what is typed.
  assert.equal(searchTextAfterNavigation("cmu ", "nova"), "nova");
  assert.equal(searchTextAfterNavigation("cmu ", ""), "");
});

Deno.test("search looks at name, summary, excerpt, committee, kind and status", () => {
  assert.ok(matchesSearch(PROJECTS[1], "navigation"));
  assert.ok(matchesSearch(PROJECTS[0], "february"));
  assert.ok(matchesSearch(PROJECTS[2], "events"));
  assert.ok(matchesSearch(PROJECTS[2], "EVENT hackathon"));
  assert.ok(matchesSearch(PROJECTS[0], "annual"));
  assert.ok(matchesSearch(PROJECTS[3], "cafe"));
  assert.ok(!matchesSearch(PROJECTS[1], "maps nova"));
  assert.ok(matchesSearch(PROJECTS[1], "   "));
  assert.equal(fold("Caf\u00E9 \u00DCber"), "cafe uber");
});

Deno.test("committee tabs count what the search finds", () => {
  const counts = committeeCounts(PROJECTS, "hack");
  assert.equal(counts.all, 2);
  assert.equal(counts.events, 2);
  assert.equal(counts.tech, 0);
  const all = committeeCounts(PROJECTS, "");
  assert.equal(all.all, 4);
  assert.equal(all.tech, 1);
  assert.deepEqual(
    filterProjects(PROJECTS, { committee: "events", q: "nova" }).map((p) => p.slug),
    ["nova"],
  );
});

Deno.test("the results line says what is shown", () => {
  assert.equal(resultsLine(38, { committee: null, q: "" }), "38 projects");
  assert.equal(resultsLine(14, { committee: "tech", q: "" }), "14 projects in Tech");
  assert.equal(
    resultsLine(4, { committee: null, q: " hack " }),
    "4 projects match \u201Chack\u201D",
  );
  assert.equal(
    resultsLine(1, { committee: "events", q: "nova" }),
    "1 project in Events matches \u201Cnova\u201D",
  );
});

Deno.test("related projects start with the same committee", () => {
  const related = relatedProjects(PROJECTS, { id: "nova", committee: "events" }, 2);
  assert.deepEqual(
    related.map((p) => p.slug),
    ["tartanhacks", "cmu-maps"],
  );
});

Deno.test("links read as short labels", () => {
  assert.equal(linkLabel("https://git.cmu.dev/ScottyLabs/maps/"), "git.cmu.dev/ScottyLabs/maps");
  assert.equal(projectPath("cmu-maps"), "/projects/cmu-maps/");
});

const DOC: ProjectDoc = {
  name: "  Nova ",
  summary: "ScottyLabs\u2019 fall hackathon.",
  description: "<p>Teams build over a weekend.</p><p>Then they demo.</p>",
  committee: "events",
  builtWith: ["tech", "events"],
  status: "annual",
  kind: "event",
  siteUrl: " ",
  repoUrl: "https://git.cmu.dev/ScottyLabs/whistle",
  videoUrl: null,
  media: [],
};

Deno.test("drafts preview as the card and page visitors will see", () => {
  const meta = { id: "1", slug: "nova", featured: false };
  const preview = draftCard(DOC, meta);
  assert.equal(preview.name, "Nova");
  assert.equal(preview.excerpt, "Teams build over a weekend. Then they demo.");
  assert.equal(preview.siteUrl, null);
  assert.equal(preview.cover, null);
  assert.equal(draftCard({ ...DOC, name: " " }, meta).name, "Untitled project");
  const page = draftDetail(DOC, meta, "2026-11-24T00:00:00Z");
  assert.deepEqual(page.builtWith, ["tech"]);
  assert.equal(page.publishedAt, "2026-11-24T00:00:00Z");
  assert.ok(sameDoc(DOC, structuredClone(DOC)));
  assert.ok(!sameDoc(DOC, { ...DOC, name: "Nova" }));
});

Deno.test("excerpts cut at a word like the server", () => {
  const long = `<p>${"word ".repeat(100)}</p>`;
  const short = excerpt(long, 30);
  assert.ok(short.endsWith("\u2026"));
  assert.ok(Array.from(short).length <= 31);
  assert.equal(excerpt("<p>Short.</p>"), "Short.");
});

Deno.test("states read the same in the list and the editor", () => {
  assert.deepEqual(stateLabel({ state: "draft", hasUnpublishedChanges: false }), {
    label: "Draft",
    tone: "neutral",
  });
  assert.equal(
    stateLabel({ state: "published", hasUnpublishedChanges: true }).label,
    "Unpublished changes",
  );
  assert.equal(stateLabel({ state: "published", hasUnpublishedChanges: false }).tone, "success");
  assert.equal(stateLabel({ state: "archived", hasUnpublishedChanges: true }).label, "Archived");
  assert.ok(isStateFilter("draft") && !isStateFilter("deleted"));
  const row: Parameters<typeof matchesRow>[0] = {
    id: "1",
    slug: "cmu-maps",
    name: "CMU Maps",
    committee: "tech",
    state: "published",
    hasUnpublishedChanges: false,
    pendingApproval: false,
    cover: null,
    hasRepo: true,
    hasSite: true,
    featured: false,
    updatedAt: "2026-11-24T00:00:00Z",
    updatedBy: null,
    canEdit: true,
    canPublish: true,
  };
  assert.ok(matchesRow(row, "maps"));
  assert.ok(matchesRow(row, "cmu-m"));
  assert.ok(!matchesRow(row, "nova"));
});

function personRow(
  person: Partial<PersonRef>,
  roles: {
    role: "lead" | "director" | "admin";
    committee: PersonRow["roles"][number]["committee"];
  }[],
): PersonRow {
  return {
    person: {
      id: person.andrewId ?? "id",
      andrewId: "someone",
      name: "Someone",
      initials: "S",
      email: null,
      hasSignedIn: true,
      lastSeenAt: null,
      ...person,
    },
    roles: roles.map((role, index) => ({
      id: `${person.andrewId ?? "id"}-${index.toString()}`,
      canPostTicker: false,
      projectIds: [],
      appointedBy: null,
      appointedAt: "2026-11-24T00:00:00Z",
      source: "portal",
      ...role,
    })),
  };
}

Deno.test("only leads of the owning committee can be chosen as editors", () => {
  const people = [
    personRow({ andrewId: "spchoksh", name: "Sara Chokshi" }, [
      { role: "lead", committee: "events" },
    ]),
    personRow({ andrewId: "evanfost", name: "Evan Foster" }, [
      { role: "director", committee: "events" },
    ]),
    personRow({ andrewId: "techlead", name: "Tech Lead" }, [
      { role: "lead", committee: "tech" },
      { role: "director", committee: "events" },
    ]),
    personRow({ andrewId: "aaron", name: "" }, [{ role: "lead", committee: "events" }]),
  ];
  const leads = leadsIn(people, "events");
  assert.deepEqual(
    leads.map((lead) => lead.andrewId),
    ["spchoksh", "aaron"],
  );
  assert.deepEqual(Object.keys(leads[0]).toSorted(), ["andrewId", "id", "initials", "name"]);
  const sara = leads[0];
  assert.deepEqual(
    leadChoices(leads, []).map((lead) => leadLabel(lead)),
    ["aaron", "Sara Chokshi (spchoksh)"],
  );
  assert.deepEqual(
    leadChoices(leads, [sara]).map((lead) => lead.andrewId),
    ["aaron"],
  );
  assert.deepEqual(leadsIn(people, "design"), []);
});

function failure(status: number): ApiError {
  return new ApiError(status, "internal", "Something went wrong.");
}

Deno.test("only dropped connections and server trouble retry a save by themselves", () => {
  assert.ok(retriesOnItsOwn(new ApiError(0, "network", "We could not reach scottylabs.org.")));
  assert.ok(retriesOnItsOwn(failure(429)));
  assert.ok(retriesOnItsOwn(failure(500)));
  assert.ok(retriesOnItsOwn(failure(503)));
  for (const status of [401, 403, 404, 409, 413, 422]) {
    assert.ok(!retriesOnItsOwn(failure(status)), `status ${status.toString()}`);
  }
  assert.ok(!retriesOnItsOwn(new Error("bug")));
});

Deno.test("the portal list says how many projects its filters leave", () => {
  assert.equal(projectCount(1), "1 project");
  assert.equal(projectCount(7), "7 projects");
  assert.equal(filterAnnouncement(3, 15), "Showing 3 of 15 projects.");
  assert.equal(filterAnnouncement(15, 15), "Showing all 15 projects.");
  assert.equal(filterAnnouncement(1, 1), "Showing all 1 project.");
  assert.equal(filterAnnouncement(0, 15), "No projects match these filters.");
});
