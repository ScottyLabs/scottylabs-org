<script lang="ts">
  // The closing invitation on the committee tint, with up to two buttons. Links to other sites open
  // in a new tab; links on this site and email links do not.
  import type { BlockByType, LinkButton } from "../../api/types.ts";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";
  import { isExternal } from "../links.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["cta"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["cta"]) => void;
  } = $props();

  const buttons = $derived(
    [
      { button: props.primary, variant: "filled" },
      { button: props.secondary, variant: "outline" },
    ].filter((entry): entry is { button: LinkButton; variant: string } => entry.button !== null && entry.button.label !== ""),
  );
</script>

<section class="cta">
  <div class="cta__copy">
    <EditableText
      as="h2"
      class="cta__heading"
      value={props.heading}
      {editor}
      {editable}
      label="Call to action heading"
      placeholder="Add a heading, such as Build with us"
      maxLength={LIMITS.heading}
      onchange={(heading) => onchange({ ...props, heading })}
    />
    <EditableText
      class="cta__text"
      value={props.text}
      {editor}
      {editable}
      label="Call to action text"
      placeholder="Say how to get involved"
      maxLength={LIMITS.shortText}
      onchange={(text) => onchange({ ...props, text })}
    />
  </div>
  {#if buttons.length > 0}
    <div class="cta__actions">
      {#each buttons as { button, variant } (variant)}
        {#if editor}
          <span class={["cb-btn", "cb-btn--lg", `cb-btn--${variant}`]}>{button.label}</span>
        {:else if isExternal(button.url)}
          <a class={["cb-btn", "cb-btn--lg", `cb-btn--${variant}`]} href={button.url} target="_blank" rel="noopener noreferrer">
            {button.label}<span class="cb-sr-only"> (opens in a new tab)</span>
          </a>
        {:else}
          <a class={["cb-btn", "cb-btn--lg", `cb-btn--${variant}`]} href={button.url}>{button.label}</a>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  .cta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 40px;
    box-sizing: border-box;
    padding: 56px;
    border-radius: 20px;
    background: var(--cb-tint);
  }
  .cta__copy {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 600px;
  }
  .cta__copy :global(.cta__heading) {
    margin: 0;
    color: var(--cb-text);
    font-size: 1.875rem;
    font-weight: 600;
    line-height: 34px;
    letter-spacing: -0.015em;
  }
  .cta__copy :global(.cta__text) {
    margin: 0;
    color: var(--cb-on-tint);
    font-size: 1.125rem;
    line-height: 28px;
  }
  .cta__actions {
    display: flex;
    flex: none;
    flex-wrap: wrap;
    gap: 10px;
  }
  .cta__actions :global(.cb-btn--outline) {
    border-color: rgba(0, 0, 0, 0.16);
    background: rgba(255, 255, 255, 0.5);
  }

  @container committee-page (width < 900px) {
    .cta {
      flex-direction: column;
      align-items: flex-start;
      gap: 24px;
      padding: 40px;
    }
  }
  @container committee-page (width < 640px) {
    .cta {
      padding: 28px 24px;
    }
    .cta__copy :global(.cta__heading) {
      font-size: 1.75rem;
      line-height: 34px;
    }
    .cta__copy :global(.cta__text) {
      font-size: 1rem;
      line-height: 24px;
    }
  }
</style>
