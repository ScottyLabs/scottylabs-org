<script lang="ts">
  // Picks one person: search people who have signed in by name or Andrew ID, or add anyone by
  // typing their Andrew ID. The choice shows as a chip that can be removed.
  //   <Field label="Person" error={errors.andrewId}>
  //     <PersonPicker bind:value={person} exclude={existingIds} />
  //   </Field>
  // An ARIA 1.2 combobox: arrow keys move through the list, Enter chooses, Escape closes.
  import { onDestroy } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import Search from "@lucide/svelte/icons/search";
  import UserPlus from "@lucide/svelte/icons/user-plus";
  import X from "@lucide/svelte/icons/x";
  import { get, isAbortError, isApiError } from "../../api/client.ts";
  import type { PeopleSearchResponse } from "../../api/types.ts";
  import Avatar from "./Avatar.svelte";
  import Spinner from "./Spinner.svelte";
  import { getFieldContext } from "./field-context.ts";
  import {
    type FoundPerson,
    type PickedPerson,
    type PickerOption,
    choosableOption,
    isKnownPerson,
    normalizeAndrewId,
    personInitials,
    personName,
    pickerOptions,
    resultsAreCurrent,
  } from "./person-picker.ts";

  const DEBOUNCE_MS = 200;

  let {
    value = $bindable(null),
    onchange,
    label,
    placeholder = "Search by name or Andrew ID",
    exclude = [],
    allowAndrewId = true,
    disabled = false,
    invalid,
    id: idProp,
  }: {
    value?: PickedPerson | null;
    onchange?: (person: PickedPerson | null) => void;
    /** The accessible name when the picker is not inside a Field. */
    label?: string;
    placeholder?: string;
    /** People who cannot be picked, by user id or Andrew ID, such as those who already have a role. */
    exclude?: readonly string[];
    /** Offers "Add <andrewid> by Andrew ID" for people the search does not know. */
    allowAndrewId?: boolean;
    disabled?: boolean;
    invalid?: boolean;
    id?: string;
  } = $props();

  const field = getFieldContext();
  const uid = $props.id();
  const inputId = $derived(idProp ?? field?.id ?? `person-picker-${uid}`);
  const listboxId = `person-picker-${uid}-listbox`;
  const isInvalid = $derived(invalid ?? field?.invalid ?? false);

  let query = $state("");
  let results = $state.raw<FoundPerson[]>([]);
  let searchedFor = $state("");
  let loading = $state(false);
  let searchError = $state<string | null>(null);
  let open = $state(false);
  let activeIndex = $state(-1);
  let announcement = $state("");
  let input: HTMLInputElement | undefined = $state();

  let timer: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | null = null;

  const typedId = $derived(allowAndrewId ? normalizeAndrewId(query) : null);
  // False from the first key press until the search for the new text answers. Until then the
  // list shows the earlier results dimmed, and none of them can be chosen.
  const settled = $derived(resultsAreCurrent(query, searchedFor, loading));
  const options = $derived(pickerOptions(results, typedId, settled, exclude));
  const expanded = $derived(open && query.trim() !== "");

  onDestroy(() => {
    clearTimeout(timer);
    controller?.abort();
  });

  function optionId(index: number): string {
    return `${listboxId}-${index.toString()}`;
  }

  function search(): void {
    clearTimeout(timer);
    controller?.abort();
    controller = null;
    activeIndex = -1;
    const q = query.trim();
    searchError = null;
    if (q === "") {
      results = [];
      searchedFor = "";
      loading = false;
      open = false;
      return;
    }
    loading = true;
    open = true;
    timer = setTimeout(() => void runSearch(q), DEBOUNCE_MS);
  }

  async function runSearch(q: string): Promise<void> {
    const current = new AbortController();
    controller = current;
    let found: FoundPerson[] = [];
    let failure: string | null = null;
    try {
      const response = await get<PeopleSearchResponse>("/people/search", {
        query: { q },
        signal: current.signal,
      });
      found = response.people;
    } catch (error) {
      if (isAbortError(error)) return;
      failure = isApiError(error) ? error.message : "The search did not work. Try again.";
    }
    // A newer search has started since; its answer is the one to show.
    if (controller !== current) return;
    controller = null;
    results = found;
    searchError = failure;
    loading = false;
    searchedFor = q;
    activeIndex = options.length > 0 ? 0 : -1;
    announcement = describeResults();
  }

  function describeResults(): string {
    if (searchError !== null) return searchError;
    const people = options.filter((option) => option.kind === "person").length;
    const extra = options.some((option) => option.kind === "andrew_id")
      ? " You can also add them by Andrew ID."
      : "";
    if (people === 0) return `No one found.${extra}`;
    return `${people.toString()} ${people === 1 ? "person" : "people"} found.${extra}`;
  }

  function choose(option: PickerOption): void {
    const next: PickedPerson =
      option.kind === "person" ? option.person : { andrewId: option.andrewId };
    value = next;
    onchange?.(next);
    announcement = `${personName(next)} chosen.`;
    query = "";
    results = [];
    searchedFor = "";
    open = false;
    activeIndex = -1;
    input?.focus();
  }

  function clear(): void {
    if (value === null) return;
    announcement = `${personName(value)} removed.`;
    value = null;
    onchange?.(null);
    input?.focus();
  }

  function optionAt(event: Event): number | null {
    const element =
      event.target instanceof Element ? event.target.closest<HTMLElement>("[data-index]") : null;
    const index = Number(element?.dataset.index ?? Number.NaN);
    return Number.isInteger(index) ? index : null;
  }

  // Keeps focus in the input, which owns the keys, so the options never take focus.
  function onListMouseDown(event: MouseEvent): void {
    event.preventDefault();
  }

  function onListMouseMove(event: MouseEvent): void {
    const index = optionAt(event);
    if (index !== null && settled) activeIndex = index;
  }

  function onListClick(event: MouseEvent): void {
    const index = optionAt(event);
    const option = index === null ? undefined : choosableOption(options, index, settled);
    if (option !== undefined) choose(option);
  }

  const pointerChoice: Attachment<HTMLElement> = (node) => {
    node.addEventListener("mousedown", onListMouseDown);
    node.addEventListener("mousemove", onListMouseMove);
    node.addEventListener("click", onListClick);
    return () => {
      node.removeEventListener("mousedown", onListMouseDown);
      node.removeEventListener("mousemove", onListMouseMove);
      node.removeEventListener("click", onListClick);
    };
  };

  function onkeydown(event: KeyboardEvent): void {
    // Earlier results cannot be moved through or chosen while the new search runs.
    const count = settled ? options.length : 0;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        if (!open) {
          open = true;
          if (activeIndex < 0 && count > 0) activeIndex = 0;
        } else if (count > 0) {
          activeIndex = (activeIndex + 1) % count;
        }
        break;
      case "ArrowUp":
        event.preventDefault();
        if (open && count > 0) activeIndex = (activeIndex - 1 + count) % count;
        break;
      case "Enter": {
        if (!expanded) break;
        // While the list is open, Enter belongs to it and does not submit the form, even when the
        // search has not answered yet.
        event.preventDefault();
        const option = choosableOption(options, activeIndex, settled);
        if (option !== undefined) choose(option);
        break;
      }
      case "Escape":
        if (expanded) {
          event.preventDefault();
          open = false;
        } else if (query !== "") {
          event.preventDefault();
          query = "";
          search();
        }
        break;
      default:
    }
  }
</script>

<div class={["picker", isInvalid && "picker--invalid", disabled && "picker--disabled"]}>
  <div class="picker__control">
    {#if value !== null}
      <span class="picker__chip">
        <Avatar initials={personInitials(value)} size="sm" />
        <span class="picker__chip-name">{personName(value)}</span>
        {#if !isKnownPerson(value)}
          <span class="picker__chip-note">by Andrew ID</span>
        {/if}
        <button
          type="button"
          class="picker__chip-remove"
          aria-label={`Remove ${personName(value)}`}
          title={`Remove ${personName(value)}`}
          {disabled}
          onclick={clear}
        >
          <X size={13} strokeWidth={2} aria-hidden="true" />
        </button>
      </span>
    {/if}
    <input
      bind:this={input}
      bind:value={query}
      id={inputId}
      class="picker__input"
      type="text"
      role="combobox"
      autocomplete="off"
      autocapitalize="none"
      spellcheck="false"
      {placeholder}
      {disabled}
      aria-label={field === undefined ? label : undefined}
      aria-describedby={field?.describedBy}
      aria-invalid={isInvalid ? "true" : undefined}
      aria-autocomplete="list"
      aria-expanded={expanded ? "true" : "false"}
      aria-controls={listboxId}
      aria-activedescendant={expanded && activeIndex >= 0 ? optionId(activeIndex) : undefined}
      oninput={search}
      {onkeydown}
      onblur={() => (open = false)}
    />
    <span class="picker__icon" aria-hidden="true">
      {#if loading}<Spinner size={14} />{:else}<Search size={15} strokeWidth={1.75} />{/if}
    </span>
  </div>

  <div class="picker__popup" hidden={!expanded}>
    <ul
      id={listboxId}
      class={["picker__list", !settled && "picker__list--stale"]}
      role="listbox"
      aria-label="People"
      aria-busy={loading ? "true" : undefined}
      {@attach pointerChoice}
    >
      {#each options as option, index (option.key)}
        <li
          id={optionId(index)}
          class={["picker__option", index === activeIndex && "picker__option--active"]}
          role="option"
          data-index={index}
          aria-selected={index === activeIndex ? "true" : "false"}
        >
          {#if option.kind === "person"}
            <Avatar initials={option.person.initials} />
            <span class="picker__option-text">
              <span class="picker__option-name">{option.person.name || option.person.andrewId}</span>
              <span class="picker__option-id">
                {option.person.andrewId}{#if !option.person.hasSignedIn}<span class="picker__option-note">
                    &middot; has not signed in yet</span
                  >{/if}
              </span>
            </span>
          {:else}
            <span class="picker__add-icon" aria-hidden="true"><UserPlus size={15} strokeWidth={1.75} /></span>
            <span class="picker__option-text">
              <span class="picker__option-name">Add {option.andrewId} by Andrew ID</span>
              <span class="picker__option-id">They get access the next time they sign in.</span>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
    {#if loading && options.length === 0}
      <p class="picker__status">Searching</p>
    {:else if searchError !== null}
      <p class="picker__status picker__status--error">{searchError}</p>
    {:else if settled && options.length === 0}
      <p class="picker__status">
        No one matches &ldquo;{query.trim()}&rdquo;. Check the spelling, or type their Andrew ID.
      </p>
    {/if}
  </div>
  <span class="p-sr-only" aria-live="polite">{announcement}</span>
</div>

<style>
  .picker {
    position: relative;
    min-width: 0;
    font-family: var(--p-font);
  }
  .picker__control {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 40px;
    padding: 3px 12px 3px 4px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
    background: var(--p-surface);
    color: var(--p-text);
    transition: border-color var(--p-duration) var(--p-ease);

    &:hover {
      border-color: #b8b8b8;
    }
    &:has(.picker__input:focus-visible) {
      border-color: var(--p-focus);
      outline: 2px solid var(--p-focus);
      outline-offset: -1px;
    }
  }
  .picker--invalid .picker__control {
    border-color: var(--p-danger);
  }
  .picker--disabled .picker__control {
    background: var(--p-surface-muted);
  }
  .picker__chip {
    display: inline-flex;
    align-items: center;
    flex: none;
    gap: 8px;
    max-width: 60%;
    height: 32px;
    padding: 0 4px 0 4px;
    border-radius: var(--p-radius-pill);
    background: var(--p-surface-muted);
  }
  .picker__chip-name {
    overflow: hidden;
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .picker__chip-note {
    color: var(--p-text-secondary);
    font-size: 0.6875rem;
    line-height: 16px;
    white-space: nowrap;
  }
  .picker__chip-remove {
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    color: var(--p-text);
    cursor: pointer;

    &:hover:not(:disabled) {
      background: var(--p-surface-strong);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 0;
    }
  }
  .picker__input {
    flex: 1;
    min-width: 120px;
    height: 32px;
    padding: 0 0 0 8px;
    border: 0;
    background: transparent;
    color: var(--p-text);
    font: inherit;
    font-size: 0.875rem;
    line-height: 20px;
    outline: none;

    &::placeholder {
      color: var(--p-placeholder);
    }
  }
  .picker__icon {
    display: grid;
    flex: none;
    place-items: center;
    color: var(--p-text-secondary);
  }
  .picker__popup {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    left: 0;
    z-index: 40;
    overflow: auto;
    max-height: 320px;
    padding: 4px;
    border: 1px solid var(--p-border-muted);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-pop);
    color: var(--p-text);

    &[hidden] {
      display: none;
    }
  }
  .picker__list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .picker__option {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 6px 8px;
    border-radius: var(--p-radius-sm);
    cursor: pointer;
  }
  .picker__list--stale .picker__option {
    cursor: default;
    opacity: 0.5;
  }
  .picker__option--active {
    background: var(--p-surface-muted);
    box-shadow: inset 0 0 0 1px var(--p-border);
  }
  .picker__option-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .picker__option-name {
    overflow: hidden;
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .picker__option-id {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .picker__option-note {
    color: var(--p-text-tertiary);
  }
  .picker__add-icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--p-surface-strong);
  }
  .picker__status {
    margin: 0;
    padding: 10px 8px;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .picker__status--error {
    color: var(--p-danger-text);
  }
</style>
