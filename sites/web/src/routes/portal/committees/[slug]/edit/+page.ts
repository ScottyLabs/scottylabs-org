import { error } from "@sveltejs/kit";
import { isCommitteeSlug } from "#lib/portal/committees.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = ({ params }) => {
  if (!isCommitteeSlug(params.slug)) error(404, "There is no committee page at that address.");
  return { slug: params.slug };
};
