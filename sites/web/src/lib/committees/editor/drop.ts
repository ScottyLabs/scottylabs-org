// Where a dragged block lands on the canvas: before the block whose edge is nearest the pointer,
// or at the end. A block that starts a row gets a line across the page above it; one beside another gets a line down its left edge.

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface DropLine {
  /** Insert before this position in the page. Equal to the number of blocks for the end. */
  before: number;
  x: number;
  y: number;
  length: number;
  vertical: boolean;
}

export interface DropLayout {
  /** The blocks, in page order, as positioned on screen. */
  blocks: readonly Rect[];
  /** The grid's own box. */
  grid: Rect;
  rowGap: number;
  columnGap: number;
}

const ROW_START_TOLERANCE = 4;

function candidates(layout: DropLayout): DropLine[] {
  const { blocks, grid, rowGap, columnGap } = layout;
  const lines: DropLine[] = [];
  // Nothing goes above the cover, so the first place is before the second block.
  for (let index = 1; index < blocks.length; index++) {
    const rect = blocks[index];
    if (rect === undefined) continue;
    const startsRow = rect.left - grid.left < ROW_START_TOLERANCE;
    lines.push(
      startsRow
        ? {
            before: index,
            x: grid.left,
            y: rect.top - rowGap / 2,
            length: grid.right - grid.left,
            vertical: false,
          }
        : {
            before: index,
            x: rect.left - columnGap / 2,
            y: rect.top,
            length: rect.bottom - rect.top,
            vertical: true,
          },
    );
  }
  const last = blocks.at(-1);
  if (last !== undefined) {
    const bottom = Math.max(...blocks.map((rect) => rect.bottom));
    lines.push({
      before: blocks.length,
      x: grid.left,
      y: Math.max(bottom, last.bottom) + rowGap / 2,
      length: grid.right - grid.left,
      vertical: false,
    });
  }
  return lines;
}

function distance(line: DropLine, x: number, y: number): number {
  if (line.vertical) {
    const nearestY = Math.min(Math.max(y, line.y), line.y + line.length);
    return Math.hypot(x - line.x, y - nearestY);
  }
  const nearestX = Math.min(Math.max(x, line.x), line.x + line.length);
  return Math.hypot(x - nearestX, y - line.y);
}

/** The drop line closest to the pointer, or null when there is nowhere to drop. */
export function dropLineAt(layout: DropLayout, x: number, y: number): DropLine | null {
  let best: DropLine | null = null;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const line of candidates(layout)) {
    const d = distance(line, x, y);
    if (d < bestDistance) {
      best = line;
      bestDistance = d;
    }
  }
  return best;
}
