<script lang="ts">
  // The width of a block on the 12-column grid, limited to the widths its type allows (spec CP-4).
  import type { Block, BlockWidth } from "../../api/types.ts";
  import { SegmentedControl, type SelectOption } from "../../portal/ui/index.ts";
  import { WIDTHS, WIDTH_LABELS, allowedWidths, blockName, widthHint } from "../blocks.ts";
  import { rowNeighbours } from "../layout.ts";

  let {
    block,
    blocks,
    onchange,
  }: {
    block: Block;
    blocks: readonly Block[];
    onchange: (width: BlockWidth) => void;
  } = $props();

  type WidthValue = "4" | "6" | "8" | "12";
  const VALUES: Readonly<Record<BlockWidth, WidthValue>> = { 4: "4", 6: "6", 8: "8", 12: "12" };

  const options = $derived<SelectOption<WidthValue>[]>(
    WIDTHS.map((width) => ({
      value: VALUES[width],
      label: WIDTH_LABELS[width],
      disabled: !allowedWidths(block.type).includes(width),
    })),
  );

  // Says what sits beside the block now, so the effect of a change is clear.
  const row = $derived.by(() => {
    const { others, free } = rowNeighbours(blocks, block.id);
    const names = others.map((other) => blockName(other));
    if (names.length > 0) return `Shares a row with ${names.join(" and ")}${free > 0 ? `, with ${free} columns free` : ""}.`;
    if (block.width < 12) return `${free} of 12 columns free beside it.`;
    return "";
  });

  function select(value: WidthValue) {
    const width = WIDTHS.find((candidate) => VALUES[candidate] === value);
    if (width !== undefined) onchange(width);
  }
</script>

<div class="width">
  <p class="width__label" aria-hidden="true">Width</p>
  <SegmentedControl
    label="Width"
    fullWidth
    options={options}
    value={VALUES[block.width]}
    onchange={select}
  />
  <p class="width__hint">{widthHint(block.type)}</p>
  {#if row !== ""}<p class="width__hint">{row}</p>{/if}
</div>

<style>
  .width {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .width__label {
    margin: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .width__hint {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
</style>
