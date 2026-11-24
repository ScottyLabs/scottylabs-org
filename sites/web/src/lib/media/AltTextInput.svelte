<script lang="ts">
  // Alt text for one photo, inside a portal Field. The server keeps it to one line of at most
  // 300 characters, so line breaks become spaces as you type.
  import TextArea from "../portal/ui/TextArea.svelte";

  /** The server's limit. */
  const MAX_ALT_LENGTH = 300;

  let {
    value,
    disabled = false,
    placeholder = "Describe the photo for screen readers",
    onchange,
  }: {
    value: string;
    disabled?: boolean;
    placeholder?: string;
    onchange: (alt: string) => void;
  } = $props();

  function oninput(event: Event & { currentTarget: HTMLTextAreaElement }): void {
    const oneLine = event.currentTarget.value.replaceAll(/[\r\n]+/gu, " ");
    if (oneLine !== event.currentTarget.value) event.currentTarget.value = oneLine;
    onchange(oneLine);
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "Enter") event.preventDefault();
  }
</script>

<TextArea
  {value}
  {disabled}
  {placeholder}
  rows={2}
  maxlength={MAX_ALT_LENGTH}
  autocomplete="off"
  {oninput}
  {onkeydown}
/>
