<script lang="ts">
  // The search box on the Projects page (Figma 50:8325, 52:12497). It filters as you type; the
  // clear button and Escape empty it.
  import Search from "@lucide/svelte/icons/search";
  import X from "@lucide/svelte/icons/x";
  import { MAX_QUERY_LENGTH } from "./filter.ts";

  let {
    value = $bindable(""),
    oninput,
  }: {
    value?: string;
    oninput: (value: string) => void;
  } = $props();

  let input: HTMLInputElement | undefined = $state();

  function clear(): void {
    value = "";
    oninput("");
    input?.focus();
  }

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && value !== "") {
      event.preventDefault();
      clear();
    }
  }
</script>

<div class="search" role="search">
  <label class="p-sr-only" for="project-search">Search projects</label>
  <div class="search__box">
    <Search class="search__icon" size={22} strokeWidth={1.75} aria-hidden="true" />
    <input
      bind:this={input}
      bind:value
      id="project-search"
      class="search__input"
      type="search"
      placeholder="Search projects"
      autocomplete="off"
      spellcheck="false"
      maxlength={MAX_QUERY_LENGTH}
      aria-describedby="project-search-hint"
      oninput={() => oninput(value)}
      {onkeydown}
    />
    {#if value !== ""}
      <button type="button" class="search__clear" aria-label="Clear search" onclick={clear}>
        <X size={20} strokeWidth={2} aria-hidden="true" />
      </button>
    {/if}
  </div>
  <p id="project-search-hint" class="search__hint">
    Try &ldquo;hackathon&rdquo;, &ldquo;maps&rdquo; or a committee such as Tech.
  </p>
</div>

<style>
  .search {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 100%;
    max-width: 640px;
    margin: 0 auto;
    font-family: var(--p-font);
  }
  .search__box {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    color: var(--p-text);

    & :global(.search__icon) {
      position: absolute;
      left: 18px;
      pointer-events: none;
    }
  }
  .search__input {
    box-sizing: border-box;
    width: 100%;
    height: 56px;
    padding: 0 52px 0 52px;
    border: 1px solid var(--p-border);
    border-radius: 100px;
    background: var(--p-surface);
    color: var(--p-text);
    font-family: inherit;
    font-size: 1.125rem;
    font-weight: 400;
    transition: border-color var(--p-duration) var(--p-ease);

    &::placeholder {
      color: var(--p-placeholder);
    }
    &:hover {
      border-color: #b8b8b8;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 1px;
      border-color: var(--p-focus);
    }
    /* Our own clear button replaces the browser's. */
    &::-webkit-search-cancel-button {
      appearance: none;
    }
  }
  .search__clear {
    position: absolute;
    right: 10px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    color: var(--p-text);

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .search__hint {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
    text-align: center;
  }
  @media (max-width: 600px) {
    .search__input {
      height: 48px;
      font-size: 1rem;
    }
  }
</style>
