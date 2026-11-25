// Activity links point at /portal/committees/<slug>/; the page lives in the editor, which opens read only for people who cannot edit it.
import { error, redirect } from "@sveltejs/kit";
import { isCommitteeSlug } from "#lib/portal/committees.ts";
import type { PageLoad } from "./$types";

export const load: PageLoad = ({ params }) => {
  if (!isCommitteeSlug(params.slug)) error(404, "There is no committee page at that address.");
  redirect(307, `/portal/committees/${params.slug}/edit/`);
};
