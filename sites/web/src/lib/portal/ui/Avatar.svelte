<script lang="ts">
  import type { TickerSource } from "../../api/types.ts";
  import { COMMITTEES } from "../committees.ts";

  let {
    initials,
    name,
    committee,
    size = "md",
    class: className,
  }: {
    initials: string;
    /** Makes the avatar an image named after the person. Leave it out when the name is shown next to it. */
    name?: string;
    /** Tints the circle with the committee color. */
    committee?: TickerSource | null;
    size?: "sm" | "md" | "lg";
    class?: string;
  } = $props();

  const tint = $derived(committee ? COMMITTEES[committee].tint : undefined);
</script>

<span
  class={["p-avatar", `p-avatar--${size}`, className]}
  style:--avatar-tint={tint}
  role={name === undefined ? undefined : "img"}
  aria-label={name}
  aria-hidden={name === undefined ? "true" : undefined}
>
  {initials.slice(0, 2).toUpperCase()}
</span>

<style>
  .p-avatar {
    box-sizing: border-box;
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 50%;
    background: var(--avatar-tint, var(--p-surface-strong));
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 500;
    letter-spacing: 0.01em;
    user-select: none;
  }
  .p-avatar--sm {
    width: 24px;
    height: 24px;
    font-size: 0.625rem;
  }
  .p-avatar--md {
    width: 28px;
    height: 28px;
    font-size: 0.75rem;
  }
  .p-avatar--lg {
    width: 40px;
    height: 40px;
    font-size: 0.875rem;
  }
</style>
