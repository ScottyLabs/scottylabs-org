// The service names problems by position, such as "blocks.3.items.1.value" or "settings.mapUrl".
// The editor keys them by block id instead, so they stay on the right block after a move.

import type { CommitteePageDoc } from "../api/types.ts";

export interface PageProblems {
  /** Problems with a block, by block id, then by the field path inside the block ("" for the block). */
  blocks: Record<string, Record<string, string>>;
  /** Problems with the page settings, by field name. */
  settings: Record<string, string>;
  /** Problems with the page as a whole, such as too many blocks. */
  page: string[];
}

export const NO_PROBLEMS: PageProblems = { blocks: {}, settings: {}, page: [] };

const BLOCK_FIELD = /^blocks\.(\d+)(?:\.(.+))?$/u;

/** Sorts the service's field messages for the document that was sent. */
export function problemsFor(
  doc: CommitteePageDoc,
  fields: Readonly<Record<string, string>>,
): PageProblems {
  const problems: PageProblems = { blocks: {}, settings: {}, page: [] };
  for (const [path, message] of Object.entries(fields)) {
    if (path.startsWith("settings.")) {
      problems.settings[path.slice("settings.".length)] = message;
      continue;
    }
    const match = BLOCK_FIELD.exec(path);
    const block = match === null ? undefined : doc.blocks[Number(match[1])];
    if (match === null || block === undefined) {
      problems.page.push(message);
      continue;
    }
    const forBlock = (problems.blocks[block.id] ??= {});
    forBlock[match[2] ?? ""] = message;
  }
  return problems;
}

export function problemCount(problems: PageProblems): number {
  const inBlocks = Object.values(problems.blocks).reduce(
    (total, fields) => total + Object.keys(fields).length,
    0,
  );
  return inBlocks + Object.keys(problems.settings).length + problems.page.length;
}

/** The first message for a field of a block, or for any field under it when `prefix` ends with ".". */
export function blockProblem(
  problems: PageProblems,
  blockId: string,
  field: string,
): string | undefined {
  return problems.blocks[blockId]?.[field];
}
