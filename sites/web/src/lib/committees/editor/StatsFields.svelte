<script lang="ts">
  // The numbers in a Stats block: two to four, each a value and what it counts.
  import GripVertical from "@lucide/svelte/icons/grip-vertical";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import type { BlockByType } from "../../api/types.ts";
  import { Button, IconButton, TextInput } from "../../portal/ui/index.ts";
  import { LIMITS, oneLine } from "../limits.ts";
  import { sortableHandle } from "./sortable.ts";

  type Item = BlockByType["stats"]["items"][number];

  let {
    items,
    problems,
    onchange,
  }: {
    items: readonly Item[];
    problems: Readonly<Record<string, string>>;
    onchange: (items: Item[], key: string | null) => void;
  } = $props();

  const uid = $props.id();

  function edit(index: number, change: Partial<Item>, field: string) {
    const current = items[index];
    if (current === undefined) return;
    onchange(items.with(index, { ...current, ...change }), `stats:${index}:${field}`);
  }

  function move(from: number, to: number, key: string | null = null) {
    if (to < 0 || to >= items.length) return;
    const next = [...items];
    const [item] = next.splice(from, 1);
    if (item === undefined) return;
    next.splice(to, 0, item);
    onchange(next, key);
  }

  // Alt with the up or down arrow moves a number while typing in it.
  function onkeydown(event: KeyboardEvent, index: number) {
    if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    const to = index + (event.key === "ArrowUp" ? -1 : 1);
    const kind = event.currentTarget instanceof HTMLElement && event.currentTarget.id.includes("-label-") ? "label" : "value";
    move(index, to);
    // Rows are keyed by position, so the number now lives in the row at `to`.
    requestAnimationFrame(() => document.querySelector<HTMLElement>(`#${CSS.escape(`${uid}-${kind}-${to}`)}`)?.focus());
  }

  let dragKey: string | null = null;

  function value(event: Event): string {
    return event.currentTarget instanceof HTMLInputElement ? oneLine(event.currentTarget.value) : "";
  }
</script>

<fieldset class="stats-fields">
  <legend class="stats-fields__legend">Numbers</legend>
  <p id={`${uid}-hint`} class="p-sr-only">Press Alt with the up or down arrow to move this number.</p>
  <ol class="stats-fields__list">
    {#each items as item, index (index)}
      {@const problem = problems[`items.${index}.value`] ?? problems[`items.${index}.label`]}
      <li class="stats-fields__row" data-sortable-item>
        <span
          class="stats-fields__handle"
          title="Drag to reorder"
          aria-hidden="true"
          {@attach sortableHandle({
            index: () => index,
            onstart: () => (dragKey = `stats-drag-${Date.now()}`),
            onmove: (from, to) => move(from, to, dragKey),
          })}
        >
          <GripVertical size={16} strokeWidth={1.75} />
        </span>
        <div class="stats-fields__inputs">
          <label class="p-sr-only" for={`${uid}-value-${index}`}>Number {index + 1}</label>
          <TextInput
            id={`${uid}-value-${index}`}
            value={item.value}
            maxlength={LIMITS.statValue}
            placeholder="15"
            aria-describedby={`${uid}-hint`}
            invalid={problems[`items.${index}.value`] !== undefined}
            oninput={(event) => edit(index, { value: value(event) }, "value")}
            onkeydown={(event) => onkeydown(event, index)}
          />
          <label class="p-sr-only" for={`${uid}-label-${index}`}>What number {index + 1} counts</label>
          <TextInput
            id={`${uid}-label-${index}`}
            value={item.label}
            maxlength={LIMITS.statLabel}
            placeholder="project teams"
            aria-describedby={`${uid}-hint`}
            invalid={problems[`items.${index}.label`] !== undefined}
            oninput={(event) => edit(index, { label: value(event) }, "label")}
            onkeydown={(event) => onkeydown(event, index)}
          />
        </div>
        <IconButton
          size="sm"
          label={`Remove number ${index + 1}`}
          icon={X}
          disabled={items.length <= LIMITS.minStats}
          onclick={() => onchange(items.filter((_, i) => i !== index), null)}
        />
        {#if problem}<p class="stats-fields__problem" role="alert">{problem}</p>{/if}
      </li>
    {/each}
  </ol>
  <div class="stats-fields__add">
    <Button
      variant="outline"
      size="sm"
      icon={Plus}
      disabled={items.length >= LIMITS.maxStats}
      onclick={() => onchange([...items, { value: "", label: "" }], null)}
    >
      Add a number
    </Button>
    <span class="stats-fields__count">{items.length} of {LIMITS.maxStats}</span>
  </div>
</fieldset>

<style>
  .stats-fields {
    margin: 0;
    padding: 0;
    border: 0;
  }
  .stats-fields__legend {
    margin-bottom: 8px;
    padding: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .stats-fields__list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0 0 12px;
    padding: 0;
    list-style: none;
  }
  .stats-fields__row {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) auto;
    align-items: center;
    gap: 4px;
  }
  .stats-fields__handle {
    display: grid;
    place-items: center;
    height: 36px;
    color: var(--p-text-tertiary);
    cursor: grab;
    touch-action: none;
  }
  .stats-fields__inputs {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: 6px;
  }
  .stats-fields__problem {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .stats-fields__add {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .stats-fields__count {
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
  }
</style>
