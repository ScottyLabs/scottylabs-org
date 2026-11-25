// Shapes the block views share.

import type { CommitteeSlug } from "../../api/types.ts";

/** The committee a page belongs to: its name and colors. */
export interface CommitteeInfo {
  slug: CommitteeSlug;
  name: string;
  /** Accent for rules and the inspector dot. */
  color: string;
  /** Background for work sessions and the call to action. */
  tint: string;
}

/**
 * How a block view is shown: on the public site ("view"), or on the editor canvas ("editor"), where
 * empty fields show placeholders and links do not navigate.
 */
export type RenderMode = "view" | "editor";

export type HeadingTag = "h1" | "h2" | "h3" | "h4";

/**
 * The tag for a heading of the page at `level`. On the editor canvas every level is one lower, so
 * the page sits under the editor's own h1 ("Tech committee page") instead of beside it.
 */
export function headingTag(level: 1, editor: boolean): "h1" | "h2";
export function headingTag(level: 2, editor: boolean): "h2" | "h3";
export function headingTag(level: 3, editor: boolean): "h3" | "h4";
export function headingTag(level: 1 | 2 | 3, editor: boolean): HeadingTag {
  const tags = ["h1", "h2", "h3", "h4"] as const;
  return tags[editor ? level : level - 1];
}
