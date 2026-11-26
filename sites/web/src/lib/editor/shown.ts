// What the rich text editor shows, kept apart from the component so it can be tested without a browser.
// Everything the editor writes into its element goes through sanitize() here: the API's value, work kept from an ended session and undo history alike. The server cleans what it stores, but rich text is cleaned again before it reaches the page, as RichText.svelte does.

import { sanitize } from "./sanitize.ts";

/** The editable element, or a stand-in in tests. */
export interface HtmlTarget {
  innerHTML: string;
}

export class ShownRichText {
  /** The value last shown or sent, as it came, so outside changes can be told from the editor's own. */
  #value: string | null = null;
  /** The cleaned markup for that value: the only thing ever written into the element. */
  #html = "";

  /** The cleaned markup last shown or recorded. */
  get html(): string {
    return this.#html;
  }

  /**
   * Shows `value` when it changed from outside, such as on load or undo. Returns whether the
   * element was rewritten.
   */
  show(element: HtmlTarget, value: string): boolean {
    if (value === this.#value) return false;
    this.#value = value;
    this.#html = sanitize(value);
    element.innerHTML = this.#html;
    return true;
  }

  /** Records markup the editor produced from its own content. Returns false when nothing changed. */
  record(value: string): boolean {
    if (value === this.#value) return false;
    this.#value = value;
    this.#html = sanitize(value);
    return true;
  }

  /**
   * Puts back exactly what was last shown or recorded, so leftovers such as an empty paragraph
   * disappear when the text loses focus. Returns whether the element was rewritten.
   */
  restore(element: HtmlTarget): boolean {
    if (element.innerHTML === this.#html) return false;
    element.innerHTML = this.#html;
    return true;
  }
}
