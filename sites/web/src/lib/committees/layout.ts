// How blocks sit on the 12-column grid. Rows fill left to right and a block that does not fit in what is left of a row starts the next one, which is what CSS grid's auto-placement does for the page. The editor uses this to find free columns and to describe moves.

import type { Block, BlockType, BlockWidth } from "../api/types.ts";
import { allowedWidths, blockName } from "./blocks.ts";

export const GRID_COLUMNS = 12;

export interface PlacedBlock {
  block: Block;
  index: number;
  /** First column, counting from 0. */
  start: number;
}

export interface GridRow {
  blocks: PlacedBlock[];
  /** Columns left empty at the end of the row. */
  free: number;
}

export function layoutRows(blocks: readonly Block[]): GridRow[] {
  const rows: GridRow[] = [];
  let current: GridRow | null = null;
  let used = 0;
  blocks.forEach((block, index) => {
    if (current === null || used + block.width > GRID_COLUMNS) {
      current = { blocks: [], free: GRID_COLUMNS };
      rows.push(current);
      used = 0;
    }
    current.blocks.push({ block, index, start: used });
    used += block.width;
    current.free = GRID_COLUMNS - used;
  });
  return rows;
}

/** Room at the end of a row where a block could go without moving anything else. */
export interface FreeSlot {
  /** Index in the page of the last block in that row; a new block goes right after it. */
  afterIndex: number;
  /** First free column, counting from 0. */
  start: number;
  columns: number;
}

/** Rows with free columns. The cover's row never has any. */
export function freeSlots(blocks: readonly Block[]): FreeSlot[] {
  return layoutRows(blocks).flatMap((row) => {
    const last = row.blocks.at(-1);
    if (last === undefined || row.free < 4) return [];
    return [{ afterIndex: last.index, start: GRID_COLUMNS - row.free, columns: row.free }];
  });
}

/** The widest of `widths` that fits in `columns`, if any. */
export function widestFitting(widths: readonly BlockWidth[], columns: number): BlockWidth | null {
  let widest: BlockWidth | null = null;
  for (const width of widths) {
    if (width <= columns && (widest === null || width > widest)) widest = width;
  }
  return widest;
}

/** The width a block of `type` gets in a free slot: the widest it may be that fits, or null when none fits. */
export function slotWidth(type: BlockType, slot: FreeSlot): BlockWidth | null {
  return type === "cover" ? null : widestFitting(allowedWidths(type), slot.columns);
}

/**
 * The blocks after moving the one with `id` into a row's free columns, sized to fill them as far
 * as its widths allow. Null when it cannot go there: the cover, a block already in that row, or
 * one with no width that fits.
 */
export function placeInSlot(blocks: readonly Block[], id: string, slot: FreeSlot): Block[] | null {
  const from = blocks.findIndex((block) => block.id === id);
  const block = blocks[from];
  if (from <= 0 || block === undefined) return null;
  const width = slotWidth(block.type, slot);
  const row = layoutRows(blocks).find((candidate) =>
    candidate.blocks.some((placed) => placed.index === slot.afterIndex),
  );
  if (width === null || row === undefined || row.blocks.some((placed) => placed.index === from)) {
    return null;
  }
  const moved = moveBlock(blocks, from, dropIndex(from, slot.afterIndex + 1));
  moved[moved.indexOf(block)] = { ...block, width };
  return moved;
}

/** The blocks after moving the one at `from` so that it ends up at `to`. Never moves the cover or puts anything above it. */
export function moveBlock(blocks: readonly Block[], from: number, to: number): Block[] {
  const target = Math.min(Math.max(to, 1), blocks.length - 1);
  const moving = blocks[from];
  if (from === 0 || moving === undefined || target === from) return [...blocks];
  const rest = blocks.filter((_, index) => index !== from);
  rest.splice(target, 0, moving);
  return rest;
}

/** Where a block dragged from `from` and dropped before position `before` ends up. */
export function dropIndex(from: number, before: number): number {
  return before > from ? before - 1 : before;
}

/** "Stats moved above What we do", after a move, naming the block after it or else the one before. */
export function moveSentence(blocks: readonly Block[], id: string): string {
  const index = blocks.findIndex((block) => block.id === id);
  const block = blocks[index];
  if (block === undefined) return "Block moved";
  const next = blocks[index + 1];
  if (next !== undefined) return `${blockName(block)} moved above ${blockName(next)}`;
  const previous = blocks[index - 1];
  return previous === undefined
    ? `${blockName(block)} moved`
    : `${blockName(block)} moved below ${blockName(previous)}`;
}

/** What shares a row with a block, for the width control's hint. */
export function rowNeighbours(
  blocks: readonly Block[],
  id: string,
): { others: Block[]; free: number } {
  const row = layoutRows(blocks).find((candidate) =>
    candidate.blocks.some((placed) => placed.block.id === id),
  );
  if (row === undefined) return { others: [], free: 0 };
  return {
    others: row.blocks.map((placed) => placed.block).filter((block) => block.id !== id),
    free: row.free,
  };
}
