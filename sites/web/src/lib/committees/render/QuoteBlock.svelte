<script lang="ts">
  // A short quote from a member, with a bar in the committee color. The page adds the quotation
  // marks, so editors type only the words.
  import type { BlockByType } from "../../api/types.ts";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["quote"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["quote"]) => void;
  } = $props();

  const showMarks = $derived(props.text !== "" || editable || editor);
</script>

{#if props.text !== "" || editor}
  <figure class="quote">
    <blockquote class="quote__text">
      {#if showMarks}<span aria-hidden="true">&ldquo;</span>{/if}<EditableText
        as="span"
        value={props.text}
        {editor}
        {editable}
        label="Quote"
        placeholder="What a member said"
        maxLength={LIMITS.shortText}
        onchange={(text) => onchange({ ...props, text })}
      />{#if showMarks}<span aria-hidden="true">&rdquo;</span>{/if}
    </blockquote>
    <EditableText
      as="figcaption"
      class="quote__attribution"
      value={props.attribution}
      {editor}
      {editable}
      label="Who said it"
      placeholder="Who said it, such as A Tech member"
      maxLength={LIMITS.attribution}
      onchange={(attribution) => onchange({ ...props, attribution })}
    />
  </figure>
{/if}

<style>
  .quote {
    display: flex;
    flex-direction: column;
    gap: 18px;
    margin: 0;
    padding: 6px 0 6px 28px;
    border-left: 3px solid var(--cb-accent);
  }
  .quote__text {
    margin: 0;
    color: var(--cb-text);
    font-size: 1.625rem;
    font-weight: 500;
    line-height: 36px;
    letter-spacing: -0.01em;
  }
  .quote :global(.quote__attribution) {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }

  @container block (width < 400px) {
    .quote__text {
      font-size: 1.25rem;
      line-height: 28px;
    }
  }
  @container committee-page (width < 640px) {
    .quote {
      padding-left: 16px;
    }
    .quote__text {
      font-size: 1.375rem;
      line-height: 30px;
    }
  }
</style>
