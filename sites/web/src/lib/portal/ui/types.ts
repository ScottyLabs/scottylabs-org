import type { LucideIcon } from "@lucide/svelte";

/** A Lucide icon component, for example `import Lock from "@lucide/svelte/icons/lock"`. */
export type IconComponent = LucideIcon;

export type ButtonVariant = "filled" | "outline" | "subtle" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface SelectOption<T extends string = string> {
  value: T;
  label: string;
  disabled?: boolean;
}

export interface MenuItem {
  label: string;
  icon?: IconComponent;
  /** A link. Links to other sites open in a new tab when `external` is set. */
  href?: string;
  external?: boolean;
  onSelect?: () => void;
  /** Destructive actions are shown in red. */
  danger?: boolean;
  disabled?: boolean;
}

export type StatusTone = "success" | "warning" | "info" | "neutral" | "danger";
