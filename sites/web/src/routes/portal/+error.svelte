<script lang="ts">
  // Errors raised by portal pages, shown inside the portal shell.
  import { page } from "$app/state";
  import PortalError from "#lib/portal/PortalError.svelte";
</script>

<PortalError status={page.status} message={page.error?.message} />
