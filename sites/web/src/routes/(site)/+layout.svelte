<script lang="ts">
  import type { Snippet } from "svelte";
  import SiteFrame from "#lib/sections/home/SiteFrame.svelte";

  let { children }: { children: Snippet } = $props();
</script>

<SiteFrame>
  {@render children()}
</SiteFrame>
