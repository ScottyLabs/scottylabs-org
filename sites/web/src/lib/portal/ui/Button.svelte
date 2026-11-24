<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLAnchorAttributes, HTMLButtonAttributes } from "svelte/elements";
  import Spinner from "./Spinner.svelte";
  import type { ButtonSize, ButtonVariant, IconComponent } from "./types.ts";

  type Props = Omit<HTMLButtonAttributes, "children"> & {
    variant?: ButtonVariant;
    size?: ButtonSize;
    /** Shown before the label. */
    icon?: IconComponent;
    /** Shows a spinner, blocks clicks and tells assistive technology the action is running. */
    loading?: boolean;
    fullWidth?: boolean;
    /** Renders a link styled as a button. */
    href?: string;
    target?: HTMLAnchorAttributes["target"];
    rel?: string;
    children: Snippet;
  };

  let {
    variant = "filled",
    size = "md",
    icon: Icon,
    loading = false,
    fullWidth = false,
    href,
    target,
    rel,
    type = "button",
    disabled = false,
    class: className,
    children,
    ...rest
  }: Props = $props();

  const iconSize = $derived(size === "lg" ? 20 : size === "md" ? 18 : 16);
  const inert = $derived(disabled || loading);
  const classes = $derived([
    "p-button",
    `p-button--${variant}`,
    `p-button--${size}`,
    fullWidth && "p-button--full",
    loading && "p-button--loading",
    className,
  ]);
  // Links that open a new tab must not hand the opener to the other page.
  const linkRel = $derived(target === "_blank" ? (rel ?? "noopener noreferrer") : rel);
</script>

{#snippet content()}
  {#if loading}
    <Spinner size={iconSize - 2} />
  {:else if Icon}
    <Icon size={iconSize} strokeWidth={1.75} aria-hidden="true" />
  {/if}
  <span class="p-button__label">{@render children()}</span>
{/snippet}

{#if href !== undefined}
  <a
    {...rest as HTMLAnchorAttributes}
    class={classes}
    href={inert ? undefined : href}
    target={inert ? undefined : target}
    rel={inert ? undefined : linkRel}
    role={inert ? "link" : undefined}
    aria-disabled={inert ? "true" : undefined}
    aria-busy={loading ? "true" : undefined}
  >
    {@render content()}
  </a>
{:else}
  <button
    {...rest}
    class={classes}
    {type}
    disabled={inert}
    aria-busy={loading ? "true" : undefined}
  >
    {@render content()}
  </button>
{/if}

<style>
  .p-button {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    gap: 8px;
    border: 1px solid transparent;
    border-radius: var(--p-radius-pill);
    font-family: var(--p-font);
    font-weight: 700;
    line-height: 20px;
    white-space: nowrap;
    text-decoration: none;
    cursor: pointer;
    user-select: none;
    transition:
      background-color var(--p-duration) var(--p-ease),
      border-color var(--p-duration) var(--p-ease),
      color var(--p-duration) var(--p-ease);

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
    &[aria-disabled="true"],
    &:disabled {
      cursor: default;
    }
  }
  .p-button--loading {
    cursor: progress;
  }
  .p-button--full {
    width: 100%;
  }
  .p-button__label {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .p-button--sm {
    height: 32px;
    padding: 0 14px;
    gap: 6px;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .p-button--md {
    height: 38px;
    padding: 0 16px;
    font-size: 0.875rem;
  }
  .p-button--lg {
    height: 48px;
    padding: 0 22px;
    font-size: 0.9375rem;
  }

  .p-button--filled {
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    &:hover {
      background: var(--p-inverse-hover);
    }
    &:active {
      background: var(--p-inverse-pressed);
    }
  }
  .p-button--outline {
    background: transparent;
    border-color: var(--p-border);
    color: var(--p-text);
    &:hover {
      background: var(--p-surface-muted);
    }
    &:active {
      background: var(--p-surface-strong);
    }
  }
  .p-button--subtle {
    background: transparent;
    color: var(--p-text);
    &:hover {
      background: var(--p-surface-muted);
    }
    &:active {
      background: var(--p-surface-strong);
    }
  }
  .p-button--danger {
    background: var(--p-danger);
    color: #ffffff;
    &:hover {
      background: var(--p-danger-hover);
    }
  }

  .p-button:disabled:not(.p-button--loading),
  .p-button[aria-disabled="true"]:not(.p-button--loading) {
    background: var(--p-surface-muted);
    border-color: var(--p-border-muted);
    color: var(--p-text-disabled);
  }
  .p-button--subtle:disabled:not(.p-button--loading) {
    background: transparent;
    border-color: transparent;
  }
  .p-button--loading.p-button--filled {
    background: var(--p-inverse-hover);
  }
</style>
