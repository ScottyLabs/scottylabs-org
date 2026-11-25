// What each block type is called, what it is for, how wide it may be and what a new one holds.

import {
  BLOCK_WIDTHS,
  type Block,
  type BlockType,
  type BlockWidth,
  MAX_BLOCKS,
} from "../api/types.ts";
import { JOIN_URL } from "./links.ts";

export const BLOCK_LABELS: Readonly<Record<BlockType, string>> = {
  cover: "Cover",
  text: "Text",
  image: "Image",
  gallery: "Gallery",
  video: "Video",
  stats: "Stats",
  teams: "Teams",
  projects: "Projects",
  quote: "Quote",
  work_sessions: "Work sessions",
  cta: "Call to action",
};

/** One sentence under the block's name in the settings panel. */
export const BLOCK_DESCRIPTIONS: Readonly<Record<BlockType, string>> = {
  cover: "The committee name, tagline, director and cover picture.",
  text: "A heading with one or more paragraphs.",
  image: "One photo with an optional caption.",
  gallery: "Three to six photos, as a mosaic or in thirds.",
  video: "A YouTube or Vimeo video that plays when someone chooses it.",
  stats: "Up to four numbers that describe the committee.",
  teams: "The committee\u2019s teams, each with its Slack channel.",
  projects: "The committee\u2019s published projects, filled in for you.",
  quote: "A short quote from a member.",
  work_sessions: "When and where the committee meets, from the page settings.",
  cta: "A closing invitation with up to two buttons.",
};

/** Every block type except the cover, which each page has exactly once. */
export type AddableType = Exclude<BlockType, "cover">;

/** The block library, in the order of the design. */
export const LIBRARY: readonly AddableType[] = [
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
];

export const WIDTHS: readonly BlockWidth[] = [4, 6, 8, 12];

/** Labels on the width control. */
export const WIDTH_LABELS: Readonly<Record<BlockWidth, string>> = {
  4: "1/3",
  6: "1/2",
  8: "2/3",
  12: "Full",
};

/** Widths in words, for hints and announcements. */
export const WIDTH_NAMES: Readonly<Record<BlockWidth, string>> = {
  4: "one third",
  6: "half",
  8: "two thirds",
  12: "full",
};

export { MAX_BLOCKS };

export function allowedWidths(type: BlockType): readonly BlockWidth[] {
  return BLOCK_WIDTHS[type];
}

/** The hint under the width control: which widths this block type may use. */
export function widthHint(type: BlockType): string {
  const widths = allowedWidths(type);
  if (widths.length === 1) return `${BLOCK_LABELS[type]} blocks are always full width.`;
  if (widths.length === WIDTHS.length) return "Snaps to the 12-column grid: 4, 6, 8 or 12 columns.";
  const names = widths.map((width) => WIDTH_NAMES[width]);
  const list = `${names.slice(0, -1).join(", ")} or ${names.at(-1) ?? ""}`;
  return `Snaps to the 12-column grid. ${BLOCK_LABELS[type]} blocks can be ${list} width.`;
}

/** How the editor names a block: a text block by its heading, anything else by its type. */
export function blockName(block: Block): string {
  if (block.type === "text" && block.props.heading.trim() !== "") return block.props.heading.trim();
  return BLOCK_LABELS[block.type];
}

/** A fresh id for a new block. The service accepts letters, digits, - and _. */
export function newBlockId(type: BlockType): string {
  const random = crypto.randomUUID().replaceAll("-", "").slice(0, 10);
  return `${type.replaceAll("_", "-")}-${random}`;
}

/** The width a new block gets: the widest it may be, except work sessions, which sit beside text. */
export function defaultWidth(type: BlockType): BlockWidth {
  if (type === "work_sessions") return 4;
  const widths = allowedWidths(type);
  return widths.at(-1) ?? 12;
}

/** A new, empty block of a type. Stats start with two numbers, the fewest a page may show. */
export function newBlock(type: AddableType, committeeName: string, width?: BlockWidth): Block {
  const id = newBlockId(type);
  const w = width ?? defaultWidth(type);
  switch (type) {
    case "text":
      return { id, type, width: w, props: { heading: "", body: "" } };
    case "image":
      return { id, type, width: w, props: { image: null, caption: "" } };
    case "gallery":
      return { id, type, width: w, props: { images: [], layout: "mosaic", caption: "" } };
    case "video":
      return { id, type, width: w, props: { url: "", title: "", poster: null } };
    case "stats":
      return {
        id,
        type,
        width: w,
        props: {
          items: [
            { value: "", label: "" },
            { value: "", label: "" },
          ],
        },
      };
    case "teams":
      return { id, type, width: w, props: { heading: "Teams", intro: "", teams: [] } };
    case "projects":
      return { id, type, width: w, props: { heading: `Projects from ${committeeName}` } };
    case "quote":
      return { id, type, width: w, props: { text: "", attribution: "" } };
    case "work_sessions":
      return { id, type, width: w, props: {} };
    case "cta":
      return {
        id,
        type,
        width: w,
        props: {
          heading: "",
          text: "",
          primary: { label: `Apply to ${committeeName}`, url: JOIN_URL },
          secondary: null,
        },
      };
    default: {
      const unknown: never = type;
      throw new Error(`There is no block type called ${String(unknown)}.`);
    }
  }
}

/** A copy of a block with a new id, to place right after it. */
export function duplicateBlock(block: Block): Block {
  return { ...structuredClone(block), id: newBlockId(block.type) };
}

export function canMove(block: Block): boolean {
  return block.type !== "cover";
}
