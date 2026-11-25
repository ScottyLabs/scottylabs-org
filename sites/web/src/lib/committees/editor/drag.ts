// What is being dragged in the editor: a new block from the library, or a block on the page.

import type { AddableType } from "../blocks.ts";

export type EditorDrag = { kind: "new"; type: AddableType } | { kind: "move"; id: string };

/** A type browsers accept for drags inside the page, so other drop targets ignore ours. */
export const DRAG_TYPE = "application/x-scottylabs-block";
