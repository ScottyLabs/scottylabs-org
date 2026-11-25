<script lang="ts">
  // A labelled text setting with a character count. Multi-line ones still save as one line, as
  // the service requires, but wrap while typing.
  import { Field, TextArea, TextInput } from "../../portal/ui/index.ts";
  import { oneLine } from "../limits.ts";

  let {
    label,
    value,
    max,
    onchange,
    help,
    error,
    placeholder,
    optional = false,
    multiline = false,
    type = "text",
    prefix,
  }: {
    label: string;
    value: string;
    max?: number;
    onchange: (value: string) => void;
    help?: string;
    error?: string | null;
    placeholder?: string;
    optional?: boolean;
    multiline?: boolean;
    type?: "text" | "url";
    /** Shown before the value, such as "#" for Slack channels. */
    prefix?: string;
  } = $props();

  const counter = $derived(max === undefined ? undefined : { length: value.length, max });

  function oninput(event: Event) {
    const target = event.currentTarget;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      onchange(oneLine(target.value));
    }
  }
</script>

<Field {label} {help} {error} {counter} {optional}>
  {#if multiline}
    <TextArea {value} rows={3} {placeholder} {oninput} />
  {:else if prefix}
    <div class="prefixed">
      <span class="prefixed__mark" aria-hidden="true">{prefix}</span>
      <TextInput {value} {type} {placeholder} {oninput} class="prefixed__input" spellcheck="false" autocapitalize="off" />
    </div>
  {:else}
    <TextInput {value} {type} {placeholder} {oninput} />
  {/if}
</Field>

<style>
  .prefixed {
    position: relative;
  }
  .prefixed__mark {
    position: absolute;
    top: 50%;
    left: 12px;
    z-index: 1;
    color: var(--p-text-secondary);
    font-size: 0.9375rem;
    transform: translateY(-50%);
    pointer-events: none;
  }
  .prefixed :global(.prefixed__input input) {
    padding-left: 26px;
  }
</style>
