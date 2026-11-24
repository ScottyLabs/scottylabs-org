// Restricted rich text: bold, italic, links and lists.
//   import { RichTextEditor, RichText, sanitize, plainTextLength } from "#lib/editor/index.ts";

export { default as RichText } from "./RichText.svelte";
export { default as RichTextEditor } from "./RichTextEditor.svelte";
export { linkFromInput, linkText, type LinkInputResult } from "./links.ts";
export {
  LINK_REL,
  LINK_TARGET,
  escapeHtml,
  normalizeRichText,
  plainText,
  plainTextLength,
  safeHref,
  sanitize,
  textToHtml,
} from "./sanitize.ts";
