<script lang="ts">
  // A centered modal dialog. Focus stays inside while it is open, Escape and the backdrop close it
  // (unless `dismissible` is false), and focus returns to whatever opened it.
  import type { ComponentProps } from "svelte";
  import Modal from "./Modal.svelte";

  let { open = $bindable(false), ...rest }: Omit<ComponentProps<typeof Modal>, "variant"> = $props();
</script>

<Modal bind:open variant="dialog" {...rest} />
