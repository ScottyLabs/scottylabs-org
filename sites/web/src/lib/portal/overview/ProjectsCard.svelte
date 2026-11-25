<script lang="ts">
  // The Projects card: a few covers, what is published and what is still a draft.
  import SquareKanban from "@lucide/svelte/icons/square-kanban";
  import type { OverviewResponse, Permissions } from "../../api/types.ts";
  import SectionCard from "./SectionCard.svelte";
  import Thumbs from "./Thumbs.svelte";
  import { projectsCopy } from "./copy.ts";

  let {
    projects,
    permissions,
  }: { projects: OverviewResponse["projects"]; permissions: Permissions } = $props();

  const copy = $derived(projectsCopy(projects, permissions));
  const thumbs = $derived(
    projects.covers.map((image, index) => ({ key: `${String(index)}-${image.src}`, image })),
  );
  const adds = $derived(permissions.projectCommittees.length > 0);
</script>

{#snippet covers()}<Thumbs items={thumbs} />{/snippet}

<SectionCard icon={SquareKanban} {copy} primaryAdds={adds} visual={thumbs.length > 0 ? covers : undefined} />
