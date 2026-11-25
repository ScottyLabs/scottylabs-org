// Tests for committee pages: the grid layout, block rules, undo history, video links, the service's field errors, publish checks and where a dragged block lands.
// Run from sites/web with `deno test tests/`.
// The type-aware linter cannot see Deno's globals or node:assert's types, so calls on them read as untyped here.
/* oxlint-disable typescript/no-unsafe-call, typescript/no-unsafe-member-access */

import assert from "node:assert/strict";
import type { Block, BlockWidth, CommitteePageDoc } from "../src/lib/api/types.ts";
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
import { publishProblems } from "../src/lib/committees/editor/publish-checks.ts";
import { problemCount, problemsFor } from "../src/lib/committees/errors.ts";
import { History } from "../src/lib/committees/history.ts";
import {
  dropIndex,
  freeSlots,
  layoutRows,
  moveBlock,
  moveSentence,
  rowNeighbours,
  widestFitting,
} from "../src/lib/committees/layout.ts";
import { cleanChannel, oneLine } from "../src/lib/committees/limits.ts";
import { videoEmbed } from "../src/lib/committees/video.ts";

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

Deno.test("videos embed from YouTube and Vimeo without tracking", () => {
  assert.deepEqual(videoEmbed("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), {
    provider: "YouTube",
    src: "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&rel=0",
  });
  assert.equal(videoEmbed("https://youtu.be/dQw4w9WgXcQ")?.provider, "YouTube");
  assert.equal(videoEmbed("https://youtube.com/shorts/dQw4w9WgXcQ")?.provider, "YouTube");
  assert.deepEqual(videoEmbed("https://vimeo.com/76979871"), {
    provider: "Vimeo",
    src: "https://player.vimeo.com/video/76979871?autoplay=1&dnt=1",
  });
  assert.equal(videoEmbed("https://player.vimeo.com/video/76979871")?.provider, "Vimeo");
  for (const bad of [
    "",
    "http://youtube.com/watch?v=dQw4w9WgXcQ",
    "https://evil.example/watch?v=x",
    "javascript:alert(1)",
    "https://youtube.com/watch?v=<script>",
  ]) {
    assert.equal(videoEmbed(bad), null, bad);
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
