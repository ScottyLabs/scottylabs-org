<script lang="ts">
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
  import type { IconComponent } from "./types.ts";

  type Props = Omit<HTMLButtonAttributes, "children" | "aria-label"> & {
    /** What the button does, read by assistive technology and shown as a tooltip. */
    label: string;
    icon: IconComponent;
    variant?: "subtle" | "outline" | "inverse";
    size?: "sm" | "md";
    href?: string;
    target?: HTMLAnchorAttributes["target"];
  };

  let {
    label,
    icon: Icon,
    variant = "subtle",
    size = "md",
    href,
    target,
    type = "button",
    class: className,
    ...rest
  }: Props = $props();

  const classes = $derived(["p-icon-button", `p-icon-button--${variant}`, `p-icon-button--${size}`, className]);
  const iconSize = $derived(size === "sm" ? 16 : 18);
</script>

{#if href !== undefined}
  <a
    {...rest as HTMLAnchorAttributes}
    class={classes}
    {href}
    {target}
    rel={target === "_blank" ? "noopener noreferrer" : undefined}
    aria-label={label}
    title={label}
  >
    <Icon size={iconSize} strokeWidth={1.75} aria-hidden="true" />
  </a>
{:else}
  <button {...rest} class={classes} {type} aria-label={label} title={label}>
    <Icon size={iconSize} strokeWidth={1.75} aria-hidden="true" />
  </button>
{/if}

<style>
  .p-icon-button {
    box-sizing: border-box;
    display: inline-grid;
    place-items: center;
    flex: none;
    border: 1px solid transparent;
    border-radius: var(--p-radius-pill);
    color: var(--p-text);
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &:disabled {
      color: var(--p-text-disabled);
      cursor: default;
      background: transparent;
    }
  }
  .p-icon-button--md {
    width: 38px;
    height: 38px;
  }
  .p-icon-button--sm {
    width: 32px;
    height: 32px;
  }
  .p-icon-button--subtle:not(:disabled) {
    &:hover {
      background: var(--p-surface-muted);
    }
    &:active {
      background: var(--p-surface-strong);
    }
  }
  .p-icon-button--outline {
    border-color: var(--p-border);
    &:not(:disabled):hover {
      background: var(--p-surface-muted);
    }
  }
  .p-icon-button--inverse {
    color: var(--p-on-inverse);
    &:not(:disabled):hover {
      background: rgba(255, 255, 255, 0.14);
    }
    &:focus-visible {
      outline-color: #ffffff;
    }
  }
</style>
