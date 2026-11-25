<script lang="ts">
  // Confirms reading the directors from the governance repository and applying them.
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
  title="Sync directors from governance?"
  description="This reads data/committees.yaml in the governance repository and makes each director listed there the director of their committee. A director it no longer lists is removed."
  dismissible={!syncing}
>
  {#if failure !== null}<Callout tone="error" role="alert" title={failure} />{/if}
  {#snippet footer()}
    <Button variant="outline" disabled={syncing} onclick={() => (open = false)}>Cancel</Button>
    <Button loading={syncing} onclick={() => void sync()}>Sync directors</Button>
  {/snippet}
</Dialog>
