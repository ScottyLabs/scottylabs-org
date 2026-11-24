<script lang="ts">
  // Overview. The cards, "Needs attention" and "Recent activity" come from the overview track;
  // until then this greets the person.
  import { formatDay, partOfDay } from "#lib/portal/dates.ts";
  import { firstName, primaryRoleLabel } from "#lib/portal/roles.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import { Badge, PageHeader } from "#lib/portal/ui/index.ts";

  const session = getPortalSession();
  const me = $derived(session.me);
  const now = new Date();
  const role = $derived(me ? primaryRoleLabel(me.roles) : null);
</script>

<svelte:head>
  <title>Overview - ScottyLabs Portal</title>
</svelte:head>

{#if me}
  <PageHeader
    title={`Good ${partOfDay(now)}, ${firstName(me.user)}.`}
    description="Here is what is live on scottylabs.org and what needs you."
  >
    {#snippet eyebrow()}
      <span>{formatDay(now)}</span>
      {#if role}<Badge>{role}</Badge>{/if}
    {/snippet}
  </PageHeader>
{/if}
