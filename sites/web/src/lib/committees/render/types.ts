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
