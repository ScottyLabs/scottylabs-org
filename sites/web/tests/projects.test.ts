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
  isProjectDoc,
  isStateFilter,
  matchesRow,
  projectCount,
  sameDoc,
  stateLabel,
} from "../src/lib/projects/drafts.ts";
import {
  type RemoteSearch,
  awaitingRemote,
  committeeCounts,
  filterProjects,
  filtersFromParams,
  filtersQuery,
  fold,
  matchesSearch,
  narrows,
  pendingSearch,
  relatedProjects,
  remoteMatches,
  remoteQuery,
  resultsLine,
  searchKey,
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

Deno.test("searches of two or more characters also go to the service", () => {
  assert.equal(remoteQuery("m"), null);
  assert.equal(remoteQuery(" m "), null);
  assert.equal(remoteQuery("  "), null);
  // Folding leaves no word to look for.
  assert.equal(remoteQuery("^^"), null);
  assert.equal(remoteQuery(" ma "), "ma");
  assert.equal(remoteQuery("Caf\u00E9 forbes "), "Caf\u00E9 forbes");
  // The same words, typed differently, are one search.
  assert.equal(searchKey(" Caf\u00E9   FORBES"), "cafe forbes");
  assert.equal(searchKey("cafe forbes"), searchKey("CAFE\tForbes "));
});

Deno.test("projects the service matched in their descriptions join the local matches", () => {
  // Only the service knows "zephyrine": it is deep in CMU Maps' description.
  const answered: RemoteSearch = { key: "zephyrine", status: "done", ids: new Set(["cmu-maps"]) };
  const q = "Zephyrine ";
  assert.equal(filterProjects(PROJECTS, { committee: null, q }).length, 0);
  const extra = remoteMatches(answered, q);
  assert.deepEqual(
    filterProjects(PROJECTS, { committee: null, q }, extra).map((p) => p.slug),
    ["cmu-maps"],
  );
  assert.deepEqual(
    filterProjects(PROJECTS, { committee: "events", q }, extra).map((p) => p.slug),
    [],
  );
  const counts = committeeCounts(PROJECTS, q, extra);
  assert.equal(counts.all, 1);
  assert.equal(counts.tech, 1);
  assert.equal(counts.events, 0);

  // Local matches stay, in the order loaded, with the service's added among them.
  const hack: RemoteSearch = { key: "hack", status: "done", ids: new Set(["cafe", "nova"]) };
  assert.deepEqual(
    filterProjects(PROJECTS, { committee: null, q: "hack" }, remoteMatches(hack, "hack")).map(
      (p) => p.slug,
    ),
    ["tartanhacks", "nova", "cafe"],
  );
  // Ids of projects the page has not loaded are left out.
  const unknown: RemoteSearch = { key: "hack", status: "done", ids: new Set(["new-one"]) };
  assert.equal(
    filterProjects(PROJECTS, { committee: null, q: "hack" }, remoteMatches(unknown, "hack")).length,
    2,
  );
});

Deno.test("an answer counts only for the search it was asked for", () => {
  const answered: RemoteSearch = { key: "zephyrine", status: "done", ids: new Set(["cmu-maps"]) };
  // Typing on changes the search; the old answer no longer applies.
  assert.equal(remoteMatches(answered, "zephyrine tiles").size, 0);
  assert.equal(remoteMatches(answered, "z").size, 0);
  assert.equal(remoteMatches({ key: "zephyrine", status: "pending" }, "zephyrine").size, 0);
  assert.equal(remoteMatches({ key: "zephyrine", status: "failed" }, "zephyrine").size, 0);
  assert.equal(remoteMatches(null, "zephyrine").size, 0);

  // While the service has not answered, "no matches" is not said yet.
  assert.ok(awaitingRemote(null, "zephyrine"));
  assert.ok(awaitingRemote({ key: "zephyrine", status: "pending" }, "ZEPHYRINE"));
  assert.ok(awaitingRemote(answered, "zephyrine tiles"));
  assert.ok(!awaitingRemote(answered, "zephyrine "));
  // A failed request falls back to the local result, and short searches never wait.
  assert.ok(!awaitingRemote({ key: "zephyrine", status: "failed" }, "zephyrine"));
  assert.ok(!awaitingRemote(null, "z"));
  assert.ok(!awaitingRemote(null, ""));
});

Deno.test("typing on keeps what the service found until it answers again", () => {
  assert.ok(narrows("zeph", "zephyrine"));
  assert.ok(narrows("zephyrine", "zephyrine tiles"));
  assert.ok(narrows("cafe forbes", "forbes cafes"));
  assert.ok(!narrows("zephyrine", "zeph"));
  assert.ok(!narrows("zephyrine tiles", "zephyrine"));
  assert.ok(!narrows("maps", "nova"));

  const answered: RemoteSearch = { key: "zeph", status: "done", ids: new Set(["cmu-maps"]) };
  // "zephyrine" can only find fewer projects than "zeph", so CMU Maps stays while it is asked.
  const typing = pendingSearch(answered, "zephyrine", PROJECTS);
  assert.equal(typing.status, "pending");
  assert.deepEqual([...remoteMatches(typing, "Zephyrine")], ["cmu-maps"]);
  assert.deepEqual(
    filterProjects(
      PROJECTS,
      { committee: null, q: "zephyrine" },
      remoteMatches(typing, "zephyrine"),
    ).map((p) => p.slug),
    ["cmu-maps"],
  );
  // The count is not announced until the service answers for the new search.
  assert.ok(awaitingRemote(typing, "zephyrine"));
  // The carried answer passes from one pending search to the next while each narrows it.
  const more = pendingSearch(typing, "zephyrine tiles", PROJECTS);
  assert.deepEqual([...remoteMatches(more, "zephyrine tiles")], ["cmu-maps"]);
  // The new answer replaces the carried one, even when it finds less.
  const none: RemoteSearch = { key: "zephyrine tiles", status: "done", ids: new Set() };
  assert.equal(remoteMatches(none, "zephyrine tiles").size, 0);

  // Only what the service alone found is carried: the cards' own matches narrow at once. For "ca",
  // CMU Maps (campus) and the cafe finder match on their cards, Nova deep in its description.
  const ca: RemoteSearch = {
    key: "ca",
    status: "done",
    ids: new Set(["cmu-maps", "cafe", "nova"]),
  };
  const cam = pendingSearch(ca, "cam", PROJECTS);
  assert.deepEqual([...remoteMatches(cam, "cam")], ["nova"]);
  assert.deepEqual(
    filterProjects(PROJECTS, { committee: null, q: "cam" }, remoteMatches(cam, "cam")).map(
      (p) => p.slug,
    ),
    ["cmu-maps", "nova"],
  );

  // Deleting letters widens the search: other projects may match, so nothing is carried.
  assert.deepEqual(pendingSearch(answered, "ze", PROJECTS), { key: "ze", status: "pending" });
  assert.deepEqual(pendingSearch(null, "zeph", PROJECTS), { key: "zeph", status: "pending" });
  // Nor after a failed request, or before the cards have loaded.
  assert.deepEqual(pendingSearch({ key: "zeph", status: "failed" }, "zephyrine", PROJECTS), {
    key: "zephyrine",
    status: "pending",
  });
  assert.equal(remoteMatches(pendingSearch(answered, "zephyrine", []), "zephyrine").size, 0);
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

Deno.test("only values shaped like a draft are put back into the editor", () => {
  assert.equal(isProjectDoc(structuredClone(DOC)), true);
  assert.equal(isProjectDoc(JSON.parse(JSON.stringify(DOC))), true);
  assert.equal(isProjectDoc(null), false);
  assert.equal(isProjectDoc([]), false);
  assert.equal(isProjectDoc({ ...DOC, name: 3 }), false);
  assert.equal(isProjectDoc({ ...DOC, committee: "marketing" }), false);
  assert.equal(isProjectDoc({ ...DOC, builtWith: ["tech", "nope"] }), false);
  assert.equal(isProjectDoc({ ...DOC, status: "sunset" }), false);
  assert.equal(isProjectDoc({ ...DOC, kind: "toString" }), false);
  assert.equal(isProjectDoc({ ...DOC, siteUrl: 1 }), false);
  assert.equal(isProjectDoc({ ...DOC, media: "cover.jpg" }), false);
  assert.equal(isProjectDoc({ ...DOC, media: [{ src: "/media/a" }] }), false);
});
