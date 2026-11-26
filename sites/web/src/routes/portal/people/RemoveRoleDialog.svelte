<script lang="ts">
  // Confirms taking a role away. The service refuses to remove your own admin role or the last
  // admin; its message is shown here as it comes.
  import { del, isApiError } from "#lib/api/client.ts";
  import { committeeName } from "#lib/portal/committees.ts";
  import { Button, Callout, Dialog } from "#lib/portal/ui/index.ts";
  import { type AssignmentRow, removedRole, rolePhrase } from "./people.ts";

  let {
    open = $bindable(false),
    row,
    /** "Remove lead" on a director's team page, "Remove role" for admins. */
    actionLabel = "Remove role",
    onremoved,
  }: {
    open?: boolean;
    row: AssignmentRow | null;
    actionLabel?: string;
    onremoved: (row: AssignmentRow, message: string) => void;
  } = $props();

  let failure = $state<string | null>(null);
  let removing = $state(false);

  let shownRow: AssignmentRow | null = null;
  $effect(() => {
    if (!open || row === shownRow) return;
    shownRow = row;
    failure = null;
    removing = false;
  });

  const title = $derived(
    row === null ? "Remove role" : `Remove ${row.person.name} as ${rolePhrase(row.assignment.role, row.assignment.committee)}?`,
  );

  const description = $derived.by(() => {
    if (row === null) return "";
    const committee = row.assignment.committee === null ? "" : committeeName(row.assignment.committee);
    switch (row.assignment.role) {
      case "admin":
        return row.assignment.source === "keycloak_group"
          ? "They lose admin access the next time they open a page, and the Keycloak admin group does not make them an admin again until they leave it or you add the role back here. To take them out of the group, change governance. If their account may be in someone else's hands, also sign them out everywhere. There must always be at least one admin."
          : "They lose admin access the next time they open a page, and stay out of the role until you add it back here, even while they are in the Keycloak admin group. There must always be at least one admin.";
      case "director":
        return `They can no longer edit the ${committee} page and projects or approve what its leads publish.`;
      case "lead":
        return "They can no longer edit their projects or write ticker announcements, from the next page they open.";
    }
  });

  async function remove() {
    if (row === null || removing) return;
    failure = null;
    removing = true;
    try {
      await del(`/people/assignments/${encodeURIComponent(row.assignment.id)}`);
      removing = false;
      open = false;
      onremoved(row, removedRole(row.person.name, row.assignment));
    } catch (error) {
      removing = false;
      failure = isApiError(error) ? error.message : "That did not work. Try again in a moment.";
    }
  }
</script>

<Dialog bind:open {title} {description} size="sm" dismissible={!removing}>
  {#if failure !== null}<Callout tone="error" role="alert" title={failure} />{/if}
  {#snippet footer()}
    <Button variant="outline" disabled={removing} onclick={() => (open = false)}>Cancel</Button>
    <Button variant="danger" loading={removing} onclick={() => void remove()}>{actionLabel}</Button>
  {/snippet}
</Dialog>
