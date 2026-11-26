<script lang="ts">
  // Confirms signing a person out everywhere (DELETE /api/people/:id/sessions), for a lost laptop
  // or a copied session. Admins only; their roles stay.
  import { del, isApiError } from "#lib/api/client.ts";
  import type { EndSessionsResponse } from "#lib/api/types.ts";
  import { Button, Callout, Dialog } from "#lib/portal/ui/index.ts";
  import { type AssignmentRow, signedOutMessage } from "./people.ts";

  let {
    open = $bindable(false),
    row,
    onsignedout,
  }: {
    open?: boolean;
    row: AssignmentRow | null;
    onsignedout: (row: AssignmentRow, message: string) => void;
  } = $props();

  let failure = $state<string | null>(null);
  let working = $state(false);

  $effect(() => {
    if (open) failure = null;
  });

  const title = $derived(row === null ? "Sign out everywhere" : `Sign ${row.person.name} out everywhere?`);
  const description = $derived(
    row?.isMe === true
      ? "This ends every session you have, in every browser, this one included. Your roles stay."
      : "This ends every session they have, in every browser. Their roles stay, and they can sign in again with their ScottyLabs account. If someone else may have their password, ask them to change it too.",
  );

  async function signOut() {
    if (row === null || working) return;
    failure = null;
    working = true;
    try {
      const result = await del<EndSessionsResponse>(
        `/people/${encodeURIComponent(row.person.id)}/sessions`,
      );
      working = false;
      open = false;
      onsignedout(row, signedOutMessage(row.person.name, result.ended));
    } catch (error) {
      working = false;
      failure = isApiError(error) ? error.message : "That did not work. Try again in a moment.";
    }
  }
</script>

<Dialog bind:open {title} {description} size="sm" dismissible={!working}>
  {#if failure !== null}<Callout tone="error" role="alert" title={failure} />{/if}
  {#snippet footer()}
    <Button variant="outline" disabled={working} onclick={() => (open = false)}>Cancel</Button>
    <Button variant="danger" loading={working} onclick={() => void signOut()}>Sign out everywhere</Button>
  {/snippet}
</Dialog>
