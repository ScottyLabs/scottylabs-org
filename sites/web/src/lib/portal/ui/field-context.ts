// Lets an input inside <Field> pick up the label's id, help and error text without wiring.

import { getContext, setContext } from "svelte";

export interface FieldContext {
  readonly id: string;
  /** The label's id, for controls a `<label for>` cannot name, such as a contenteditable. */
  readonly labelId: string;
  /** Space-separated ids of the help, counter and error text. */
  readonly describedBy: string | undefined;
  readonly invalid: boolean;
  readonly required: boolean;
}

const KEY = Symbol("portal-field");

export function setFieldContext(context: FieldContext): void {
  setContext(KEY, context);
}

/** The surrounding field, or undefined when the input is used on its own. */
export function getFieldContext(): FieldContext | undefined {
  return getContext<FieldContext | undefined>(KEY);
}
