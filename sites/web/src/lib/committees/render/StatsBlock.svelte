<script lang="ts">
  // Two to four numbers, each with a rule in the committee color.
  import type { BlockByType } from "../../api/types.ts";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["stats"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["stats"]) => void;
  } = $props();

  function setItem(index: number, change: Partial<{ value: string; label: string }>) {
    const current = props.items[index];
    if (current === undefined) return;
    onchange({ items: props.items.with(index, { ...current, ...change }) });
  }
</script>

<dl class="stats" style:--count={props.items.length}>
  {#each props.items as item, index (index)}
    <div class="stats__item">
      <dt class="stats__label-slot">
        <EditableText
          as="span"
          class="stats__label"
          value={item.label}
          {editor}
          {editable}
          label={`Label for number ${index + 1}`}
          placeholder="What it counts"
          maxLength={LIMITS.statLabel}
          onchange={(label) => setItem(index, { label })}
        />
      </dt>
      <dd class="stats__value-slot">
        <EditableText
          as="span"
          class="stats__value"
          value={item.value}
          {editor}
          {editable}
          label={`Number ${index + 1}`}
          placeholder="0"
          maxLength={LIMITS.statValue}
          onchange={(value) => setItem(index, { value })}
        />
      </dd>
    </div>
  {/each}
</dl>

<style>
  .stats {
    display: grid;
    grid-template-columns: repeat(var(--count), minmax(0, 1fr));
    gap: 24px;
    margin: 0;
  }
  .stats__item {
    display: flex;
    flex-direction: column-reverse;
    justify-content: flex-end;
    gap: 6px;
    padding: 4px 0 4px 24px;
    border-left: 2px solid var(--cb-accent);
  }
  .stats__label-slot,
  .stats__value-slot {
    margin: 0;
  }
  .stats :global(.stats__value) {
    display: block;
    color: var(--cb-text);
    font-size: 2.75rem;
    font-weight: 600;
    line-height: 50px;
    letter-spacing: -0.02em;
  }
  .stats :global(.stats__label) {
    display: block;
    color: var(--cb-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }

  @container block (width < 560px) {
    .stats :global(.stats__value) {
      font-size: 2.25rem;
      line-height: 44px;
    }
  }
  @container committee-page (width < 640px) {
    .stats {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }
    .stats__item {
      padding-left: 16px;
    }
  }
</style>
