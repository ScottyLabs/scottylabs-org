<script lang="ts">
  // Asks for a link address. Opened from the Link button or Cmd/Ctrl+K in RichTextEditor.
  import { onMount } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import Button from "../portal/ui/Button.svelte";
  import TextInput from "../portal/ui/TextInput.svelte";
  import { linkFromInput } from "./links.ts";

  let {
    id,
    initial = "",
    editing = false,
    onsave,
    onremove,
    oncancel,
  }: {
    id: string;
    /** The current address when the selection is already a link. */
    initial?: string;
    /** True when the selection is already a link, which offers "Remove link". */
    editing?: boolean;
    onsave: (href: string) => void;
    onremove: () => void;
    oncancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let address = $state(initial);
  let error = $state<string | null>(null);
  let input: HTMLInputElement | null = $state(null);

  const inputId = $derived(`${id}-input`);
  const errorId = $derived(`${id}-error`);

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function submit(event: SubmitEvent): void {
    event.preventDefault();
    const result = linkFromInput(address);
    if (result.ok) onsave(result.href);
    else error = result.message;
  }

  // Escape anywhere in the popover cancels, without also leaving the editor.
  function onkeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    oncancel();
  }

  const closeOnEscape: Attachment<HTMLElement> = (node) => {
    node.addEventListener("keydown", onkeydown);
    return () => node.removeEventListener("keydown", onkeydown);
  };
</script>

<div
  {id}
  class="rte-link"
  role="dialog"
  aria-label={editing ? "Edit link" : "Add link"}
  {@attach closeOnEscape}
>
  <form class="rte-link__form" novalidate onsubmit={submit}>
    <label class="rte-link__label" for={inputId}>Link address</label>
    <TextInput
      bind:ref={input}
      bind:value={address}
      id={inputId}
      type="url"
      inputmode="url"
      autocomplete="off"
      spellcheck="false"
      placeholder="https://"
      invalid={error !== null}
      aria-describedby={error === null ? undefined : errorId}
      oninput={() => (error = null)}
    />
    <p id={errorId} class="rte-link__error" aria-live="polite">{error ?? ""}</p>
    <div class="rte-link__actions">
      {#if editing}
        <Button variant="subtle" size="sm" onclick={onremove}>Remove link</Button>
      {/if}
      <Button variant="outline" size="sm" onclick={oncancel}>Cancel</Button>
      <Button size="sm" type="submit">{editing ? "Update link" : "Add link"}</Button>
    </div>
  </form>
</div>

<style>
  .rte-link {
    box-sizing: border-box;
    width: min(340px, calc(100vw - 32px));
    padding: 12px;
    border: 1px solid var(--p-border-muted);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-pop);
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .rte-link__form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .rte-link__label {
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .rte-link__error {
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;

    &:empty {
      margin-top: -8px;
    }
  }
  .rte-link__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
