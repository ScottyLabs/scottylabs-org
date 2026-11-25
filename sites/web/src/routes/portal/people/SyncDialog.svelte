<script lang="ts">
  // Confirms reading the directors and heads from the governance repository and applying them.
  import { isApiError, post } from "#lib/api/client.ts";
  import type { GovernanceSyncResponse } from "#lib/api/types.ts";
  import { Button, Callout, Dialog } from "#lib/portal/ui/index.ts";

  let {
    open = $bindable(false),
    onsynced,
  }: { open?: boolean; onsynced: (result: GovernanceSyncResponse) => void } = $props();

  let failure = $state<string | null>(null);
  let syncing = $state(false);

  $effect(() => {
    if (open) failure = null;
  });

  async function sync() {
    if (syncing) return;
    failure = null;
    syncing = true;
    try {
      const result = await post<GovernanceSyncResponse>("/people/sync-governance");
      syncing = false;
      open = false;
      onsynced(result);
    } catch (error) {
      syncing = false;
      failure = isApiError(error) ? error.message : "That did not work. Try again in a moment.";
    }
  }
</script>

<Dialog
  bind:open
  title="Sync directors and heads from governance?"
  description="This reads data/committees.yaml in the governance repository. Each director listed there becomes director of their committee, and each head becomes a lead who can write ticker announcements. Leads governance no longer lists are removed unless they were edited here. People you removed or changed here are not added back."
  dismissible={!syncing}
>
  {#if failure !== null}<Callout tone="error" role="alert" title={failure} />{/if}
  {#snippet footer()}
    <Button variant="outline" disabled={syncing} onclick={() => (open = false)}>Cancel</Button>
    <Button loading={syncing} onclick={() => void sync()}>Sync directors and heads</Button>
  {/snippet}
</Dialog>
