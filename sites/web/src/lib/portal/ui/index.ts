// The portal UI kit. Import components from here:
//   import { Button, Dialog, Field, TextInput } from "#lib/portal/ui/index.ts";
// Tokens live in ./tokens.css (loaded by the root layout); table classes in ./table.css (loaded by the portal layout).

export { default as Avatar } from "./Avatar.svelte";
export { default as Badge } from "./Badge.svelte";
export { default as Button } from "./Button.svelte";
export { default as Callout } from "./Callout.svelte";
export { default as Card } from "./Card.svelte";
export { default as Checkbox } from "./Checkbox.svelte";
export { default as CommitteeChip } from "./CommitteeChip.svelte";
export { default as Dialog } from "./Dialog.svelte";
export { default as DogMark } from "./DogMark.svelte";
export { default as EmptyState } from "./EmptyState.svelte";
export { default as Field } from "./Field.svelte";
export { default as IconButton } from "./IconButton.svelte";
export { default as KeptChangesNotice } from "./KeptChangesNotice.svelte";
export { default as Menu, type MenuTriggerProps } from "./Menu.svelte";
export { default as PageHeader } from "./PageHeader.svelte";
export { default as PersonPicker } from "./PersonPicker.svelte";
export { default as SegmentedControl } from "./SegmentedControl.svelte";
export { default as Select } from "./Select.svelte";
export { default as Sheet } from "./Sheet.svelte";
export { default as Skeleton } from "./Skeleton.svelte";
export { default as Spinner } from "./Spinner.svelte";
export { default as StatusDot } from "./StatusDot.svelte";
export { default as Switch } from "./Switch.svelte";
export { default as Tabs, type TabItem } from "./Tabs.svelte";
export { default as TextArea } from "./TextArea.svelte";
export { default as TextInput } from "./TextInput.svelte";
export { default as ToastRegion } from "./ToastRegion.svelte";
export { getFieldContext, type FieldContext } from "./field-context.ts";
export { lockScroll } from "./scroll-lock.ts";
export {
  type PickedPerson,
  isKnownPerson,
  normalizeAndrewId,
  personInitials,
  personName,
} from "./person-picker.ts";
export type {
  ButtonSize,
  ButtonVariant,
  IconComponent,
  MenuItem,
  SelectOption,
  StatusTone,
} from "./types.ts";
