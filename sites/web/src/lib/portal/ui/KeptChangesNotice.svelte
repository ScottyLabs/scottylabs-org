<script lang="ts">
  // Shown when an editor opens with work kept from a session that ended (lib/portal/kept-changes.ts)
  // but the record has changed since, so the work is not put back without asking.
  import Button from "./Button.svelte";
  import Callout from "./Callout.svelte";

  let {
    subject,
    onrestore,
    ondiscard,
  }: {
    /** What changed since, for example "this project". */
    subject: string;
    onrestore: () => void;
    ondiscard: () => void;
  } = $props();
</script>

<Callout tone="warning" title="You have changes that were not saved" role="status">
  They were kept when your session ended, but {subject} has changed since. Putting them back
  replaces what is there now.
  {#snippet actions()}
    <Button variant="outline" size="sm" onclick={onrestore}>Put my changes back</Button>
    <Button variant="subtle" size="sm" onclick={ondiscard}>Discard them</Button>
  {/snippet}
</Callout>
