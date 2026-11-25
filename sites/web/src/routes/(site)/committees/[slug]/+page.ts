// Committee pages are prerendered for the seven committees; their content loads in the browser from the published version, so a publish shows without a rebuild.
import { error } from "@sveltejs/kit";
import { COMMITTEE_SLUGS } from "#lib/api/types.ts";
import { isCommitteeSlug } from "#lib/portal/committees.ts";
import type { EntryGenerator, PageLoad } from "./$types";

export const entries: EntryGenerator = () => COMMITTEE_SLUGS.map((slug) => ({ slug }));

// The page writes its own meta description, so the root layout leaves out the site's default.
export const load: PageLoad = ({ params }) => {
  if (!isCommitteeSlug(params.slug)) error(404, "There is no committee with that name.");
  return { slug: params.slug, ownDescription: true };
};
