// Undo steps for photo fields. Photo fields report every key press in an alt text box as a new photo value, so these name the field to group those changes under, which makes one description one undo step, like typing in any other field. Choosing, replacing, reordering or removing a photo gets null, so it stays a step of its own.

import type { ImageRef } from "../../api/types.ts";

/** `${field}.alt` when only the alt text of the photo in `field` changed, otherwise null. */
export function altTextField(
  field: string,
  before: ImageRef | null,
  after: ImageRef | null,
): string | null {
  const onlyAlt =
    before !== null && after !== null && before.src === after.src && before.alt !== after.alt;
  return onlyAlt ? `${field}.alt` : null;
}

/** `${field}.<position>.alt` when only the alt text of one photo in a list changed, otherwise null. */
export function altTextFieldInList(
  field: string,
  before: readonly ImageRef[],
  after: readonly ImageRef[],
): string | null {
  if (before.length !== after.length) return null;
  let changed: number | null = null;
  for (const [position, image] of after.entries()) {
    const previous = before[position];
    if (previous === undefined || previous.src !== image.src) return null;
    if (previous.alt === image.alt) continue;
    if (changed !== null) return null;
    changed = position;
  }
  return changed === null ? null : `${field}.${changed.toString()}.alt`;
}
