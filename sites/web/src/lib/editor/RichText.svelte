<script lang="ts">
  // Shows restricted rich text from the API, cleaned again here before it reaches the page.
  import "./rich-text.css";
  import { sanitize } from "./sanitize.ts";

  let {
    html,
    class: className,
  }: {
    /** Rich text from a text block or a project description. */
    html: string;
    class?: string;
  } = $props();

  const clean = $derived(sanitize(html));
</script>

<!-- Safe: sanitize() keeps only p, br, strong, em, ul, ol, li and checked https or mailto links. -->
<div class={["rich-text", className]}>{@html clean}</div>
