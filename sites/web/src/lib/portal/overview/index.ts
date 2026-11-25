// The pieces of the portal overview (src/routes/portal/+page.svelte).
//   import { TickerCard, AttentionList } from "#lib/portal/overview/index.ts";

export { default as ActivityList } from "./ActivityList.svelte";
export { default as AttentionList } from "./AttentionList.svelte";
export { default as CommitteesCard } from "./CommitteesCard.svelte";
export { default as OverviewSkeleton } from "./OverviewSkeleton.svelte";
export { default as ProjectsCard } from "./ProjectsCard.svelte";
export { default as SubmissionsList } from "./SubmissionsList.svelte";
export { default as TickerCard } from "./TickerCard.svelte";
export {
  NEW_PROJECT_HREF,
  activityText,
  activityTime,
  committeesCopy,
  editedAgo,
  greeting,
  lastEditedText,
  overviewDescription,
  projectsCopy,
  tickerCopy,
  type CardAction,
  type CardCopy,
} from "./copy.ts";
