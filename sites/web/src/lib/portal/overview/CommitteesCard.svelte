<script lang="ts">
  // The Committee pages card: covers of the pages, how many are published and which have changes
  // waiting. A director with one committee sees "Your committee page" (Figma 106:4591).
  import Users from "@lucide/svelte/icons/users";
  import type { OverviewResponse, Permissions } from "../../api/types.ts";
  import { COMMITTEES } from "../committees.ts";
  import SectionCard from "./SectionCard.svelte";
  import Thumbs from "./Thumbs.svelte";
  import { committeesCopy } from "./copy.ts";

  let {
    committees,
    permissions,
    viewerId,
    now,
  }: {
    committees: OverviewResponse["committees"];
    permissions: Permissions;
    viewerId: string;
    now: Date;
  } = $props();

  const copy = $derived(committeesCopy(committees, permissions, viewerId, now));
  const thumbs = $derived(
    copy.covers.map((entry) => ({
      key: entry.slug,
      image: entry.cover,
      tint: COMMITTEES[entry.slug].tint,
    })),
  );
</script>

<SectionCard icon={Users} {copy}>
  {#snippet visual()}<Thumbs items={thumbs} wide={copy.single} />{/snippet}
</SectionCard>
