<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  let {
    as = "div",
    padding = "md",
    elevated = false,
    class: className,
    children,
    ...rest
  }: HTMLAttributes<HTMLElement> & {
    as?: "div" | "section" | "article" | "li" | "aside";
    /** none for tables and lists that run edge to edge. */
    padding?: "none" | "sm" | "md" | "lg";
    /** Adds the soft shadow used for cards over artwork. */
    elevated?: boolean;
    children: Snippet;
  } = $props();
</script>

<svelte:element
  this={as}
  {...rest}
  class={["p-card", `p-card--pad-${padding}`, elevated && "p-card--elevated", className]}
>
  {@render children()}
</svelte:element>

<style>
  .p-card {
    box-sizing: border-box;
    min-width: 0;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .p-card--pad-none {
    overflow: hidden;
  }
  .p-card--pad-sm {
    padding: 14px 16px;
  }
  .p-card--pad-md {
    padding: 20px;
  }
  .p-card--pad-lg {
    padding: 28px 32px;
    @media (max-width: 600px) {
      padding: 22px 20px;
    }
  }
  .p-card--elevated {
    box-shadow: var(--p-shadow-card);
  }
</style>
