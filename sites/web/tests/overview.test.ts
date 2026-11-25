/// <reference lib="dom" />
// Tests for the overview, approvals and People wording and rules. Run from sites/web with `deno test tests/`.
// The DOM library reference types the rich text code the approval preview uses; the tests never use the DOM.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type {
  ActivityEntry,
  Approval,
  CommitteeSlug,
  OverviewResponse,
  Permissions,
  PersonRow,
  ProjectAdminSummary,
  ProjectState,
  RoleAssignment,
} from "../src/lib/api/types.ts";
import {
  approvedMessage,
  linkHost,
  projectCardFromDoc,
  reviewTitle,
  sentBackMessage,
  submissionStatus,
  submissionSubject,
} from "../src/lib/portal/approvals/format.ts";
import {
  activityText,
  activityTime,
  committeesCopy,
  greeting,
  overviewDescription,
  projectsCopy,
  tickerCopy,
} from "../src/lib/portal/overview/copy.ts";
import {
  appointedText,
  assignmentRows,
  canEditText,
  matchingAssignment,
  nowHasRole,
  projectNames,
  projectsFor,
  removedRole,
  withPerson,
  withoutAssignment,
} from "../src/routes/portal/people/people.ts";

const NOW = new Date("2026-11-24T20:00:00Z");

const SARA = { id: "u-sara", andrewId: "spchoksh", name: "Sara Chokshi", initials: "SC" };
const EVAN = { id: "u-evan", andrewId: "evanfost", name: "Evan Foster", initials: "EF" };

function permissions(overrides: Partial<Permissions>): Permissions {
  return {
    canUsePortal: true,
    isAdmin: false,
    directorOf: [],
    leadOf: [],
    tickerSources: [],
    tickerNeedsApproval: false,
    tickerManages: [],
    editablePages: [],
    projectCommittees: [],
    editableProjectIds: [],
    approvesFor: [],
    peopleScope: [],
    ...overrides,
  };
}

const ADMIN = permissions({
  isAdmin: true,
  tickerSources: ["club", "events", "tech"],
  projectCommittees: ["events", "tech"],
  peopleScope: "all",
});
const DIRECTOR = permissions({
  directorOf: ["events"],
  tickerSources: ["events"],
  tickerManages: ["events"],
  editablePages: ["events"],
  projectCommittees: ["events"],
  approvesFor: ["events"],
  peopleScope: ["events"],
});
const LEAD = permissions({
  leadOf: ["events"],
  tickerSources: ["events"],
  tickerNeedsApproval: true,
  editableProjectIds: ["p-nova"],
});

function approval(overrides: Partial<Approval>): Approval {
  return {
    id: "a-1",
    status: "waiting",
    note: null,
    decidedBy: null,
    decidedAt: null,
    subjectType: "announcement",
    subjectId: "n-1",
    committee: "events",
    submittedBy: SARA,
    submittedAt: "2026-11-24T15:20:00Z",
    announcement: {
      kind: "event",
      committee: "events",
      message: "Volunteer sign-ups for TartanHacks are open.",
      linkUrl: "https://www.tartanhacks.com/volunteer",
      expiresAt: "2026-12-04T22:00:00Z",
    },
    project: null,
    href: "/portal/ticker/n-1/",
    canDecide: true,
    ...overrides,
  };
}

const NOVA_DOC = {
  name: "Nova",
  summary: "A hackathon for first-timers",
  description: `<p>${"Build something new. ".repeat(20)}</p>`,
  committee: "events" as const,
  builtWith: [],
  status: "live" as const,
  kind: "event" as const,
  siteUrl: "https://nova.scottylabs.org",
  repoUrl: null,
  videoUrl: null,
  media: [{ src: "/seed/projects/nova.png", alt: "Nova", width: 1200, height: 900 }],
};

Deno.test("the review dialog names the lead and what they sent", () => {
  const post = approval({});
  assert.equal(reviewTitle(post), "Sara Chokshi wants to post this");
  assert.equal(approvedMessage(post), "Approved. It is on the ticker now.");
  assert.equal(sentBackMessage(post), "Sent back to Sara with your note.");
  const project = approval({ subjectType: "project", announcement: null, project: NOVA_DOC });
  assert.equal(reviewTitle(project), "Sara Chokshi wants to publish Nova");
  assert.equal(approvedMessage(project), "Approved. Nova is on the site now.");
  assert.equal(reviewTitle(approval({ submittedBy: null })), "A lead wants to post this");
  assert.equal(
    sentBackMessage(approval({ submittedBy: null })),
    "Sent back to the lead with your note.",
  );
});

Deno.test("links show their host, and only https links count", () => {
  assert.equal(linkHost("https://www.tartanhacks.com/volunteer"), "tartanhacks.com");
  assert.equal(linkHost("https://cmumaps.com"), "cmumaps.com");
  assert.equal(linkHost("javascript:alert(1)"), null);
  assert.equal(linkHost("http://example.com"), null);
  assert.equal(linkHost(null), null);
  assert.equal(linkHost("not a url"), null);
});

Deno.test("a submitted project previews as its card, without a page yet", () => {
  const card = projectCardFromDoc("p-nova", NOVA_DOC);
  assert.equal(card.id, "p-nova");
  assert.equal(card.slug, "");
  assert.equal(card.cover?.src, "/seed/projects/nova.png");
  assert.ok(card.excerpt.length <= 241);
  assert.ok(card.excerpt.endsWith("\u2026"));
  assert.ok(!card.excerpt.includes("<p>"));
});

Deno.test("leads see where each submission stands", () => {
  assert.deepEqual(submissionStatus(approval({}), NOW), {
    tone: "info",
    label: "Waiting",
    detail: "Sent Nov 24",
  });
  const sentBack = approval({
    status: "sent_back",
    decidedBy: EVAN,
    decidedAt: "2026-11-20T15:00:00Z",
    note: "Add the date.",
  });
  assert.deepEqual(submissionStatus(sentBack, NOW), {
    tone: "warning",
    label: "Sent back",
    detail: "By Evan Foster, Nov 20",
  });
  assert.equal(
    submissionStatus(approval({ status: "approved", decidedBy: EVAN }), NOW).label,
    "Approved",
  );
  assert.equal(submissionSubject(approval({})), "Volunteer sign-ups for TartanHacks are open.");
  assert.equal(
    submissionSubject(approval({ subjectType: "project", announcement: null, project: NOVA_DOC })),
    "Changes to Nova",
  );
});

const TICKER: OverviewResponse["ticker"] = {
  live: 5,
  slots: 6,
  mineLive: 1,
  nextToExpire: { id: "n-2", message: "Foundry fireside chat.", expiresAt: "2026-11-26T20:30:00Z" },
};

Deno.test("the greeting and the line under it follow the role", () => {
  assert.equal(greeting("Thomas", NOW), "Good afternoon, Thomas.");
  assert.equal(
    overviewDescription(ADMIN),
    "Here is what is live on scottylabs.org and what needs you.",
  );
  assert.equal(
    overviewDescription(DIRECTOR),
    "You can edit Events. Here is what is live and what needs you.",
  );
  assert.equal(
    overviewDescription(LEAD),
    "Here is your work on scottylabs.org and what needs you.",
  );
});

Deno.test("the ticker card counts slots and says what leaves next", () => {
  const admin = tickerCopy(TICKER, ADMIN, NOW);
  assert.equal(admin.meta, "5 of 6 slots");
  assert.equal(admin.headline, "5 announcements live");
  assert.equal(admin.text, "Next to leave: Foundry fireside chat, in 2 days.");
  assert.equal(admin.primary?.href, "/portal/ticker/new/");
  assert.equal(admin.blocked, null);

  const director = tickerCopy(TICKER, DIRECTOR, NOW);
  assert.equal(director.headline, "1 of yours is live");
  assert.equal(director.text, "Foundry fireside chat leaves in 2 days.");

  const full = { ...TICKER, live: 6, mineLive: 2 };
  assert.equal(tickerCopy(full, DIRECTOR, NOW).headline, "2 of yours are live");
  assert.equal(
    tickerCopy(full, DIRECTOR, NOW).blocked,
    "All 6 slots are in use. End one to post another.",
  );
  // A lead's post waits for approval anyway, so a full ticker does not stop them.
  assert.equal(tickerCopy(full, LEAD, NOW).blocked, null);
  const none = { ...TICKER, mineLive: 0, nextToExpire: null };
  assert.equal(tickerCopy(none, LEAD, NOW).headline, "None of yours is live");
  assert.equal(tickerCopy(none, permissions({ leadOf: ["tech"] }), NOW).primary, null);
});

const COMMITTEES: OverviewResponse["committees"] = {
  total: 7,
  published: 7,
  withUnpublishedChanges: ["tech"],
  editable: ["events", "outreach", "tech", "labrador", "design", "finance", "foundry"],
  covers: [
    { slug: "events", cover: null },
    { slug: "tech", cover: null },
  ],
};

Deno.test("the committee card speaks to what the viewer edits", () => {
  const admin = committeesCopy(COMMITTEES, ADMIN);
  assert.equal(admin.title, "Committee pages");
  assert.equal(admin.headline, "7 pages published");
  assert.equal(admin.text, "Tech has changes that are not published yet.");
  assert.equal(admin.covers.length, 2);

  const own = committeesCopy(
    { ...COMMITTEES, editable: ["events"], withUnpublishedChanges: [] },
    DIRECTOR,
  );
  assert.equal(own.title, "Your committee page");
  assert.equal(own.meta, "/committees/events");
  assert.equal(own.headline, "Events page is published");
  assert.equal(own.primary?.label, "Edit the Events page");
  assert.equal(own.primary?.href, "/portal/committees/events/");
  assert.deepEqual(
    own.covers.map((entry) => entry.slug),
    ["events"],
  );
  assert.ok(own.single);

  const changed = committeesCopy(
    { ...COMMITTEES, editable: ["events"], withUnpublishedChanges: ["events"] },
    DIRECTOR,
  );
  assert.equal(changed.headline, "Events page has unpublished changes");

  const lead = committeesCopy(
    { ...COMMITTEES, published: 6, editable: [], withUnpublishedChanges: [] },
    LEAD,
  );
  assert.equal(lead.headline, "6 of 7 pages published");
  assert.match(lead.text, /^Directors edit/u);
});

const PROJECTS: OverviewResponse["projects"] = {
  total: 16,
  published: 14,
  drafts: 2,
  inScope: 16,
  covers: [],
};

Deno.test("the projects card counts what is published", () => {
  const admin = projectsCopy(PROJECTS, ADMIN);
  assert.equal(admin.meta, "16 projects");
  assert.equal(admin.headline, "14 published, 2 drafts");
  assert.equal(admin.text, "2 drafts are not on the site yet.");
  assert.equal(admin.primary?.label, "Add project");

  const director = projectsCopy({ ...PROJECTS, published: 2, drafts: 0, inScope: 2 }, DIRECTOR);
  assert.equal(director.headline, "2 Events projects");
  assert.equal(director.text, "Both are published.");
  assert.equal(
    projectsCopy({ ...PROJECTS, drafts: 1, inScope: 3 }, DIRECTOR).text,
    "1 is still a draft.",
  );

  const lead = projectsCopy({ ...PROJECTS, inScope: 1, published: 1, drafts: 0 }, LEAD);
  assert.equal(lead.title, "Your projects");
  assert.equal(lead.headline, "1 project to edit");
  assert.equal(lead.primary?.href, "/portal/projects/");
  assert.equal(projectsCopy({ ...PROJECTS, inScope: 0 }, LEAD).primary, null);
});

Deno.test("activity reads as a sentence with you for the viewer", () => {
  const entry: ActivityEntry = {
    id: "e-1",
    actor: EVAN,
    summary: "Published the Events page",
    at: "2026-11-24T16:42:00Z",
    href: null,
  };
  assert.equal(activityText(entry, "u-thomas"), "Evan Foster published the Events page");
  assert.equal(activityText(entry, EVAN.id), "You published the Events page");
  assert.equal(activityText({ ...entry, actor: null }, "x"), "Published the Events page");
  assert.equal(activityTime("2026-11-24T16:42:00Z", NOW), "11:42 AM");
  assert.equal(activityTime("2026-11-23T16:42:00Z", NOW), "Yesterday");
  assert.equal(activityTime("2026-11-20T16:42:00Z", NOW), "Nov 20");
});

function role(overrides: Partial<RoleAssignment>): RoleAssignment {
  return {
    id: "r-1",
    role: "lead",
    committee: "events",
    canPostTicker: true,
    projectIds: [],
    appointedBy: null,
    appointedAt: "2026-09-14T15:00:00Z",
    source: "portal",
    ...overrides,
  };
}

function person(who: typeof SARA, roles: RoleAssignment[]): PersonRow {
  return { person: { ...who, email: null, hasSignedIn: true, lastSeenAt: null }, roles };
}

function projectSummary(
  id: string,
  name: string,
  committee: CommitteeSlug,
  state: ProjectState,
): ProjectAdminSummary {
  return {
    id,
    slug: id.slice(2),
    name,
    committee,
    state,
    hasUnpublishedChanges: false,
    pendingApproval: false,
    cover: null,
    hasRepo: false,
    hasSite: false,
    featured: false,
    updatedAt: "2026-11-01T00:00:00Z",
    updatedBy: null,
    canEdit: true,
    canPublish: true,
  };
}

const PROJECT_LIST: ProjectAdminSummary[] = [
  projectSummary("p-nova", "Nova", "events", "published"),
  projectSummary("p-tartanhacks", "TartanHacks", "events", "published"),
  projectSummary("p-old", "Old thing", "events", "archived"),
  projectSummary("p-maps", "CMU Maps", "tech", "published"),
];

Deno.test("people are listed one role per row, admins first", () => {
  const people = [
    person(SARA, [
      role({ id: "r-sara", projectIds: ["p-nova"] }),
      role({ id: "r-sara-tech", committee: "tech" }),
    ]),
    person(EVAN, [role({ id: "r-evan", role: "director" })]),
    person({ ...EVAN, id: "u-thomas", name: "Thomas Kanz" }, [
      role({ id: "r-thomas", role: "admin", committee: null }),
    ]),
  ];
  const all = assignmentRows(people, "u-thomas", "all");
  assert.deepEqual(
    all.map((row) => row.assignment.id),
    ["r-thomas", "r-evan", "r-sara", "r-sara-tech"],
  );
  assert.ok(all[0]?.isMe);
  const team = assignmentRows(people, "u-evan", ["events"]);
  assert.deepEqual(
    team.map((row) => row.assignment.id),
    ["r-evan", "r-sara"],
  );

  const removed = withoutAssignment(people, "r-evan");
  assert.equal(removed.length, 2);
  const updated = withPerson(people, person(SARA, [role({ id: "r-sara" })]));
  assert.equal(updated.find((row) => row.person.id === SARA.id)?.roles.length, 1);
  assert.equal(withPerson(people, person({ ...SARA, id: "u-new" }, [role({})])).length, 4);
});

Deno.test("each role says what it can edit and who gave it", () => {
  const names = projectNames(PROJECT_LIST);
  assert.equal(canEditText(role({ role: "admin", committee: null }), names), "Everything");
  assert.equal(
    canEditText(role({ role: "director" }), names),
    "Events page, Events projects, ticker",
  );
  assert.equal(
    canEditText(role({ projectIds: ["p-tartanhacks", "p-nova"] }), names),
    "Nova, TartanHacks, ticker",
  );
  assert.equal(canEditText(role({ projectIds: ["p-nova", "p-x"] }), null), "2 projects, ticker");
  assert.equal(canEditText(role({ canPostTicker: false }), names), "Nothing yet");

  assert.equal(appointedText(role({}), "u-evan", NOW), "By an admin");
  assert.equal(appointedText(role({ appointedBy: EVAN }), "u-evan", NOW), "By you, Sep 14");
  assert.equal(
    appointedText(role({ appointedBy: EVAN }), "u-thomas", NOW),
    "By Evan Foster, Sep 14",
  );
  assert.equal(appointedText(role({ source: "governance" }), "u-evan", NOW), "From governance");

  assert.deepEqual(
    projectsFor(PROJECT_LIST, "events").map((p) => p.name),
    ["Nova", "TartanHacks"],
  );
  assert.deepEqual(projectsFor(PROJECT_LIST, null), []);
});

Deno.test("confirmations name the person and the role", () => {
  assert.equal(nowHasRole("Helen Fu", "lead", "events"), "Helen Fu is now a lead for Events.");
  assert.equal(nowHasRole("Helen Fu", "director", "tech"), "Helen Fu is now director of Tech.");
  assert.equal(nowHasRole("Helen Fu", "admin", null), "Helen Fu is now an admin.");
  assert.equal(removedRole("Sara Chokshi", role({})), "Removed Sara Chokshi as a lead in Events.");
  const row = person(SARA, [
    role({ id: "r-a" }),
    role({ id: "r-b", role: "director", committee: "tech" }),
  ]);
  assert.equal(matchingAssignment(row, "director", "tech")?.id, "r-b");
  assert.equal(matchingAssignment(row, "admin", null), undefined);
});
