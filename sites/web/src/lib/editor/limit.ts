// The character limit of a rich text editor: when text is over it, and what to tell people.

/** True when `length` visible characters is more than `maxChars` allows. */
export function isOverLimit(length: number, maxChars?: number): boolean {
  return maxChars !== undefined && length > maxChars;
}

/** What to do about text over the limit, shown under the editor. */
export function limitMessage(maxChars: number): string {
  const unit = maxChars === 1 ? "character" : "characters";
  return `Shorten this to ${maxChars.toLocaleString("en-US")} ${unit} or fewer.`;
}

/**
 * Whether the editor shows as invalid: the parent says so, the text is over its limit, or the
 * surrounding Field has an error. Passing `invalid={false}` never hides text over the limit.
 */
export function editorInvalid(state: {
  invalid?: boolean;
  over: boolean;
  fieldInvalid?: boolean;
}): boolean {
  return (state.invalid ?? false) || state.over || (state.fieldInvalid ?? false);
}
