/// <reference lib="dom" />
// Tests for committee pages: the grid layout, block rules, heading levels, undo history, the service's field errors, publish checks, where a dragged block lands, when the editor asks for the edit lock and how handles report moves.
// Run from sites/web with `deno test tests/`.
// The DOM library reference types the browser code under test; the tests stand in for the DOM where they need it.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type { Block, BlockWidth, CommitteePageDoc, ImageRef } from "../src/lib/api/types.ts";
import {
  BLOCK_LABELS,
  blockName,
  defaultWidth,
  duplicateBlock,
  newBlock,
  newBlockId,
  widthHint,
} from "../src/lib/committees/blocks.ts";
import { dropLineAt } from "../src/lib/committees/editor/drop.ts";
import { HEARTBEAT_MS, LOCK_MS, LockClock } from "../src/lib/committees/editor/lock-clock.ts";
import { altTextField, altTextFieldInList } from "../src/lib/committees/editor/photo-undo.ts";
import { publishProblems } from "../src/lib/committees/editor/publish-checks.ts";
import { sortableHandle } from "../src/lib/committees/editor/sortable.ts";
import { problemCount, problemsFor } from "../src/lib/committees/errors.ts";
import { History } from "../src/lib/committees/history.ts";
import {
  dropIndex,
  freeSlots,
  layoutRows,
  moveBlock,
  moveSentence,
  placeInSlot,
  rowNeighbours,
  slotWidth,
  widestFitting,
} from "../src/lib/committees/layout.ts";
import { cleanChannel, oneLine } from "../src/lib/committees/limits.ts";
import { headingTag } from "../src/lib/committees/render/types.ts";

function cover(): Block {
  return {
    id: "cover",
    type: "cover",
    width: 12,
    props: {
      tagline: "We build",
      image: { src: "/seed/photos/c10.jpg", alt: "Students", width: 1000, height: 667 },
    },
  };
}

function text(id: string, width: BlockWidth, heading = ""): Block {
  return { id, type: "text", width, props: { heading, body: "" } };
}

function sessions(id = "sessions", width: BlockWidth = 4): Block {
  return { id, type: "work_sessions", width, props: {} };
}

function stats(id = "stats"): Block {
  return {
    id,
    type: "stats",
    width: 12,
    props: {
      items: [
        { value: "15", label: "teams" },
        { value: "2011", label: "since" },
      ],
    },
  };
}

function doc(blocks: Block[]): CommitteePageDoc {
  return {
    blocks,
    settings: {
      indexDescription: "",
      teamsLine: "",
      meetingTime: "",
      meetingRoom: "",
      mapUrl: null,
      slackChannel: null,
    },
  };
}

const ids = (blocks: readonly Block[]) => blocks.map((block) => block.id);

Deno.test("rows fill left to right and wrap when a block does not fit", () => {
  const blocks = [
    cover(),
    text("what", 8),
    sessions(),
    text("wide", 12),
    text("a", 6),
    text("b", 4),
    text("c", 4),
  ];
  const rows = layoutRows(blocks);
  assert.deepEqual(
    rows.map((row) => row.blocks.map((placed) => `${placed.block.id}@${placed.start}`)),
    [["cover@0"], ["what@0", "sessions@8"], ["wide@0"], ["a@0", "b@6"], ["c@0"]],
  );
  assert.deepEqual(
    rows.map((row) => row.free),
    [0, 0, 0, 2, 8],
  );
});

Deno.test("free slots are rows with room for at least a third", () => {
  const blocks = [cover(), text("what", 12, "What we do"), sessions(), text("a", 12)];
  assert.deepEqual(freeSlots(blocks), [{ afterIndex: 2, start: 4, columns: 8 }]);
  // Two columns left over are too few for any block.
  assert.deepEqual(freeSlots([cover(), sessions(), text("a", 6), text("b", 12)]), []);
  assert.equal(widestFitting([4, 6, 8, 12], 8), 8);
  assert.equal(widestFitting([6, 8, 12], 4), null);
  assert.equal(widestFitting([8, 12], 8), 8);
});

Deno.test("moves keep the cover first and describe themselves", () => {
  const blocks = [cover(), text("what", 8, "What we do"), sessions(), stats()];
  const moved = moveBlock(blocks, 3, 1);
  assert.deepEqual(ids(moved), ["cover", "stats", "what", "sessions"]);
  assert.equal(moveSentence(moved, "stats"), "Stats moved above What we do");
  assert.equal(moveSentence(moveBlock(blocks, 1, 3), "what"), "What we do moved below Stats");
  assert.deepEqual(ids(moveBlock(blocks, 2, 0)), ["cover", "sessions", "what", "stats"]);
  assert.deepEqual(ids(moveBlock(blocks, 0, 2)), ids(blocks));
  // Dropping before a later block lands one place earlier, because the block left its old place.
  assert.equal(dropIndex(1, 3), 2);
  assert.equal(dropIndex(3, 1), 1);
});

Deno.test("the width hint names the widths a block may use", () => {
  assert.equal(widthHint("text"), "Snaps to the 12-column grid: 4, 6, 8 or 12 columns.");
  assert.equal(
    widthHint("stats"),
    "Snaps to the 12-column grid. Stats blocks can be half, two thirds or full width.",
  );
  assert.equal(widthHint("cta"), "Call to action blocks are always full width.");
  const { others, free } = rowNeighbours(
    [cover(), text("what", 8, "What we do"), sessions()],
    "sessions",
  );
  assert.deepEqual(ids(others), ["what"]);
  assert.equal(free, 0);
});

Deno.test("new blocks start empty but valid to save", () => {
  for (const type of [
    "text",
    "image",
    "gallery",
    "video",
    "stats",
    "teams",
    "projects",
    "quote",
    "work_sessions",
    "cta",
  ] as const) {
    const block = newBlock(type, "Tech");
    assert.equal(block.type, type);
    assert.equal(block.width, defaultWidth(type));
    assert.match(block.id, /^[A-Za-z0-9_-]{1,64}$/u);
  }
  const statsBlock = newBlock("stats", "Tech");
  assert.ok(statsBlock.type === "stats" && statsBlock.props.items.length === 2);
  const projects = newBlock("projects", "Tech");
  assert.ok(projects.type === "projects" && projects.props.heading === "Projects from Tech");
  assert.equal(defaultWidth("work_sessions"), 4);
  assert.notEqual(newBlockId("text"), newBlockId("text"));
  const copy = duplicateBlock(stats());
  assert.notEqual(copy.id, "stats");
  assert.deepEqual(copy.props, stats().props);
});

Deno.test("blocks are named by heading, or by type", () => {
  assert.equal(blockName(text("a", 12, "  What we do ")), "What we do");
  assert.equal(blockName(text("a", 12)), "Text");
  assert.equal(blockName(sessions()), BLOCK_LABELS.work_sessions);
});

Deno.test("history undoes and redoes, merging quick typing in one field", () => {
  const history = new History<string>(3);
  history.record("a", null, 0);
  history.record("ab", "title", 100);
  history.record("abc", "title", 400);
  history.record("abcd", "title", 2000);
  assert.equal(history.undo("abcde"), "abcd");
  assert.equal(history.undo("abcd"), "ab");
  assert.equal(history.redo("ab"), "abcd");
  assert.equal(history.undo("abcd"), "ab");
  assert.equal(history.undo("ab"), "a");
  assert.equal(history.undo("a"), null);
  assert.ok(history.canRedo);
  history.record("x");
  assert.ok(!history.canRedo);

  const long = new History<number>(2);
  for (const n of [1, 2, 3]) long.record(n);
  assert.equal(long.undo(4), 3);
  assert.equal(long.undo(3), 2);
  assert.equal(long.undo(2), null);
});

Deno.test("history keeps at least the last 20 changes", () => {
  const history = new History<number>();
  for (let n = 0; n < 25; n++) history.record(n);
  let current = 25;
  for (let step = 0; step < 20; step++) {
    const previous = history.undo(current);
    assert.notEqual(previous, null);
    current = previous ?? current;
  }
});

Deno.test("field errors from the service land on the right block", () => {
  const page = doc([cover(), stats("numbers"), text("what", 8)]);
  const problems = problemsFor(page, {
    "blocks.1.items.0.value": "Fill in this number.",
    "blocks.0.image.alt": "Describe the image.",
    "settings.mapUrl": "Links must start with https://",
    blocks: "Start the page with its cover.",
    "blocks.9.width": "Out of range",
  });
  assert.deepEqual(problems.blocks.numbers, { "items.0.value": "Fill in this number." });
  assert.deepEqual(problems.blocks.cover, { "image.alt": "Describe the image." });
  assert.deepEqual(problems.settings, { mapUrl: "Links must start with https://" });
  assert.deepEqual(problems.page, ["Start the page with its cover.", "Out of range"]);
  assert.equal(problemCount(problems), 5);
});

Deno.test("publishing needs alt text, a cover picture and blocks that are filled in", () => {
  assert.deepEqual(publishProblems(doc([cover(), stats()])), []);
  const noCover: Block = {
    id: "cover",
    type: "cover",
    width: 12,
    props: { tagline: "", image: null },
  };
  const photo: Block = {
    id: "photo",
    type: "image",
    width: 6,
    props: {
      image: {
        src: "/media/0190e0a4-7c1b-7d2e-9f00-000000000001",
        alt: " ",
        width: 100,
        height: 100,
      },
      caption: "",
    },
  };
  const gallery: Block = {
    id: "gallery",
    type: "gallery",
    width: 12,
    props: { images: [], layout: "mosaic", caption: "" },
  };
  const video: Block = {
    id: "video",
    type: "video",
    width: 6,
    props: { url: "", title: "", poster: null },
  };
  const teams: Block = {
    id: "teams",
    type: "teams",
    width: 12,
    props: { heading: "Teams", intro: "", teams: [] },
  };
  const problems = publishProblems(doc([noCover, photo, gallery, video, teams]));
  assert.deepEqual(
    problems.map((problem) => `${problem.blockId}:${problem.field}`),
    ["cover:image", "photo:image.alt", "gallery:images", "video:url", "teams:teams"],
  );
  assert.equal(
    problems[1]?.message,
    "Describe the photo in Image 1 for people using screen readers.",
  );
});

function videoBlock(url: string): Block {
  return { id: "video", type: "video", width: 6, props: { url, title: "Channel", poster: null } };
}

Deno.test("publishing needs a link to one video, not a channel", () => {
  assert.deepEqual(publishProblems(doc([cover(), videoBlock("https://youtu.be/dQw4w9WgXcQ")])), []);
  const problems = publishProblems(
    doc([cover(), videoBlock("https://www.youtube.com/@scottylabs")]),
  );
  assert.deepEqual(
    problems.map((problem) => `${problem.blockId}:${problem.field}`),
    ["video:url"],
  );
  assert.equal(
    problems[0]?.message,
    "Use a link to one YouTube or Vimeo video, not a channel or playlist.",
  );
});

function withCover(image: ImageRef): Block {
  return { id: "cover", type: "cover", width: 12, props: { tagline: "", image } };
}

function upload(width: number | null, alt = "Students at work"): ImageRef {
  return {
    src: "/media/0190e0a4-7c1b-7d2e-9f00-000000000001",
    alt,
    width,
    height: width === null ? null : Math.round(width * 0.6),
  };
}

Deno.test("an uploaded cover must be at least 1,200 pixels wide to publish", () => {
  // Wide enough, or of unknown size, or bundled with the site: nothing to fix.
  assert.deepEqual(publishProblems(doc([withCover(upload(1200)), stats()])), []);
  assert.deepEqual(publishProblems(doc([withCover(upload(1920)), stats()])), []);
  assert.deepEqual(publishProblems(doc([withCover(upload(null)), stats()])), []);
  assert.deepEqual(publishProblems(doc([cover(), stats()])), []);

  const narrow = publishProblems(doc([withCover(upload(960)), stats()]));
  assert.deepEqual(
    narrow.map((problem) => `${problem.blockId}:${problem.field}`),
    ["cover:image"],
  );
  assert.equal(
    narrow[0]?.message,
    "Use a cover photo at least 1,200 pixels wide. This one is 960 pixels wide.",
  );
  // A narrow cover without a description needs both fixed, the width first, as the service lists them.
  assert.deepEqual(
    publishProblems(doc([withCover(upload(1199, " ")), stats()])).map((problem) => problem.field),
    ["image", "image.alt"],
  );
});

Deno.test("a block dropped in free columns fills them", () => {
  // Rows: cover; What we do (12); Work sessions (4) with 8 free; Image (12).
  const photo: Block = {
    id: "photo",
    type: "image",
    width: 12,
    props: { image: null, caption: "" },
  };
  const blocks = [cover(), text("what", 12, "What we do"), sessions(), photo];
  const [slot] = freeSlots(blocks);
  assert.ok(slot !== undefined);
  assert.equal(slotWidth("image", slot), 8);
  assert.equal(slotWidth("work_sessions", slot), 6);
  assert.equal(slotWidth("cover", slot), null);
  // The image moves up beside Work sessions and takes the 8 free columns.
  const moved = placeInSlot(blocks, "photo", slot);
  assert.deepEqual(
    moved?.map((block) => `${block.id}:${block.width}`),
    ["cover:12", "what:12", "sessions:4", "photo:8"],
  );
  assert.deepEqual(
    layoutRows(moved ?? []).map((row) => row.blocks.map((placed) => placed.block.id)),
    [["cover"], ["what"], ["sessions", "photo"]],
  );
  // A block from earlier on the page lands after Work sessions too.
  assert.deepEqual(ids(placeInSlot(blocks, "what", slot) ?? []), [
    "cover",
    "sessions",
    "what",
    "photo",
  ]);
  // Not the cover, not a block already in that row, and not a block too wide for the space.
  assert.equal(placeInSlot(blocks, "cover", slot), null);
  assert.equal(placeInSlot(blocks, "sessions", slot), null);
  assert.equal(
    placeInSlot([cover(), text("what", 12), sessions(), stats()], "stats", { ...slot, columns: 4 }),
    null,
  );
});

Deno.test("the editor keeps the edit lock only while the person is working", () => {
  const start = 1_000_000;
  const clock = new LockClock(start);
  assert.equal(clock.due, true, "asks for the lock when the editor opens");
  clock.claimed(start);
  assert.equal(clock.due, false, "an idle editor does not refresh the lock");
  assert.equal(clock.touch(start + 30_000), false);
  assert.equal(clock.due, true, "a click or a key press since the last request refreshes it");
  clock.claimed(start + HEARTBEAT_MS);
  assert.equal(clock.due, false);
  // Back after the lock lapsed: ask again straight away.
  assert.equal(clock.touch(start + HEARTBEAT_MS + LOCK_MS), true);
  clock.claimed(start + HEARTBEAT_MS + LOCK_MS);
  assert.equal(clock.touch(start + HEARTBEAT_MS + LOCK_MS + 1000), false);
});

Deno.test("typing alt text is one undo step, choosing a photo is its own", () => {
  const a = { src: "/media/a", alt: "", width: 10, height: 10 };
  const b = { src: "/media/b", alt: "", width: 10, height: 10 };
  assert.equal(altTextField("image", a, { ...a, alt: "S" }), "image.alt");
  assert.equal(altTextField("image", a, b), null);
  assert.equal(altTextField("image", null, a), null);
  assert.equal(altTextField("image", a, null), null);
  assert.equal(altTextFieldInList("images", [a, b], [a, { ...b, alt: "S" }]), "images.1.alt");
  assert.equal(altTextFieldInList("images", [a, b], [b, a]), null);
  assert.equal(altTextFieldInList("images", [a], [a, b]), null);
  assert.equal(altTextFieldInList("images", [a, b], [a, b]), null);
});

/** Stands in for browser calls the tests do not need. */
function ignore(): void {
  // Nothing to do.
}

/** A pointer event at a height on the page. */
function pointer(clientY: number) {
  return { button: 0, pointerId: 1, clientY, preventDefault: ignore };
}

/** A list of three rows with a drag handle on the first, laid out 40 px apart, and a stand-in window. */
function sortableList(onend: (moved: boolean) => void) {
  const listeners = new Map<string, (event: unknown) => void>();
  const fakeWindow = {
    addEventListener: (type: string, listener: (event: unknown) => void) =>
      listeners.set(type, listener),
    removeEventListener: (type: string) => listeners.delete(type),
  };
  const rows = [0, 1, 2].map((index) => ({
    getBoundingClientRect: () => ({ top: index * 40, height: 40 }),
  }));
  const list = { querySelectorAll: () => rows };
  let press: ((event: unknown) => void) | undefined;
  const handle = {
    closest: () => ({ parentElement: list }),
    addEventListener: (_type: string, listener: (event: unknown) => void) => (press = listener),
    removeEventListener: ignore,
  };
  let index = 0;
  const detach = sortableHandle({
    index: () => index,
    onmove: (_from, to) => {
      index = to;
    },
    onend,
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- Deno has no DOM, so a stand-in plays the handle.
  })(handle as unknown as HTMLElement);
  return {
    fakeWindow,
    press: (clientY: number) => press?.(pointer(clientY)),
    drag: (clientY: number) => listeners.get("pointermove")?.(pointer(clientY)),
    release: (clientY: number) => listeners.get("pointerup")?.(pointer(clientY)),
    detach,
  };
}

Deno.test("a drag handle reports a move only when the item moved", () => {
  const ends: boolean[] = [];
  const previous: unknown = Reflect.get(globalThis, "window");
  const list = sortableList((moved) => {
    ends.push(moved);
  });
  Reflect.set(globalThis, "window", list.fakeWindow);
  try {
    // A click on the handle.
    list.press(20);
    list.release(20);
    // A drag past the middle of the next row.
    list.press(20);
    list.drag(70);
    list.release(70);
    // Down and back up again.
    list.press(60);
    list.drag(10);
    list.drag(70);
    list.release(70);
  } finally {
    list.detach();
    Reflect.set(globalThis, "window", previous);
  }
  assert.deepEqual(ends, [false, true, false]);
});

Deno.test("a dragged block lands before the nearest edge", () => {
  const layout = {
    grid: { left: 0, top: 0, right: 1200, bottom: 1000 },
    rowGap: 72,
    columnGap: 24,
    blocks: [
      { left: 0, top: 0, right: 1200, bottom: 500 },
      { left: 0, top: 572, right: 792, bottom: 800 },
      { left: 816, top: 572, right: 1200, bottom: 800 },
    ],
  };
  // Above the second row: a line across the page before block 1.
  assert.deepEqual(dropLineAt(layout, 400, 540), {
    before: 1,
    x: 0,
    y: 536,
    length: 1200,
    vertical: false,
  });
  // Between the two blocks of the row: a line down the left edge of block 2.
  assert.equal(dropLineAt(layout, 805, 700)?.before, 2);
  assert.equal(dropLineAt(layout, 805, 700)?.vertical, true);
  // Below everything: the end.
  assert.equal(dropLineAt(layout, 600, 900)?.before, 3);
  // Never above the cover.
  assert.equal(dropLineAt(layout, 600, -100)?.before, 1);
  assert.equal(dropLineAt({ ...layout, blocks: [] }, 0, 0), null);
});

Deno.test("plain text stays on one line and channels lose their #", () => {
  assert.equal(oneLine("a\nb\r\nc"), "a b c");
  assert.equal(cleanChannel("  ##Tech-Maps "), "tech-maps");
});

Deno.test("the page's headings sit one level lower on the editor canvas", () => {
  assert.equal(headingTag(1, false), "h1");
  assert.equal(headingTag(2, false), "h2");
  assert.equal(headingTag(3, false), "h3");
  // The editor's own title is the h1, so the cover is an h2 there.
  assert.equal(headingTag(1, true), "h2");
  assert.equal(headingTag(2, true), "h3");
  assert.equal(headingTag(3, true), "h4");
});
