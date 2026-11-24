<script lang="ts">
  // A bottom sheet for phones: the same behavior as Dialog, anchored to the bottom of the screen.
  import type { ComponentProps } from "svelte";
  import Modal from "./Modal.svelte";

  let { open = $bindable(false), ...rest }: Omit<ComponentProps<typeof Modal>, "variant"> = $props();
</script>

<Modal bind:open variant="sheet" {...rest} />
