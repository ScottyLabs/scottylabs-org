<script lang="ts">
  // A heading with paragraphs of restricted rich text. In the editor the body is edited in place
  // with the inline rich text editor and its format bar, which the editor canvas passes in as
  // `textEditor`, so public committee pages never load the editor's code.
  import type { BlockByType } from "../../api/types.ts";
  import RichText from "../../editor/RichText.svelte";
  import type RichTextEditor from "../../editor/RichTextEditor.svelte";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";
  import { headingTag } from "./types.ts";

  let {
    props,
    editor,
    editable,
    textEditor: TextEditor,
    onchange,
  }: {
    props: BlockByType["text"];
    editor: boolean;
    editable: boolean;
    /** The inline rich text editor, from the editor canvas. */
    textEditor?: typeof RichTextEditor;
    onchange: (props: BlockByType["text"]) => void;
  } = $props();
</script>

<section class="text">
  <EditableText
    as={headingTag(2, editor)}
    class="cb-heading"
    value={props.heading}
    {editor}
    {editable}
    label="Heading"
    placeholder="Add a heading"
    maxLength={LIMITS.heading}
    onchange={(heading) => onchange({ ...props, heading })}
  />
  {#if editable && TextEditor !== undefined}
    <TextEditor
      class="text__body text__body--editing"
      variant="inline"
      label={props.heading === "" ? "Text" : `Text under ${props.heading}`}
      value={props.body}
      maxChars={LIMITS.body}
      placeholder="Write a few sentences."
      onchange={(body) => onchange({ ...props, body })}
    />
  {:else if props.body !== ""}
    <RichText class="text__body" html={props.body} />
  {:else if editor}
    <p class="text__body cb-placeholder">Write a few sentences.</p>
  {/if}
</section>

<style>
  .text {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .text :global(.text__body) {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 1.125rem;
    font-weight: 400;
    line-height: 28px;
  }
  .text :global(.text__body p),
  .text :global(.text__body ul),
  .text :global(.text__body ol) {
    margin-bottom: 16px;
    font-size: inherit;
    line-height: inherit;
  }
  .text :global(.text__body a) {
    color: var(--cb-text);
  }
  .text :global(.text__body--editing) {
    --rte-inline-focus-outline: 2px solid var(--p-focus);
  }

  @container committee-page (width < 640px) {
    .text :global(.text__body) {
      font-size: 1rem;
      line-height: 26px;
    }
  }
</style>
