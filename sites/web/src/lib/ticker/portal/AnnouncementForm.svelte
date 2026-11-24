<script lang="ts">
  // The composer (spec TK-4 to TK-8): type, committee, message, link and expiry, with the
  // announcement at real size between two dog marks and when it will be on the ticker.
  // Admins and directors publish; leads send it to their director for approval. On phones the
  // main button stays in view at the bottom.
  import Link from "@lucide/svelte/icons/link";
  import { tick } from "svelte";
  import { isApiError } from "../../api/client.ts";
  import type {
    Announcement,
    AnnouncementInput,
    AnnouncementKind,
    AnnouncementListResponse,
    PersonRef,
    TickerSource,
  } from "../../api/types.ts";
  import { committeeName, getPortalSession } from "../../portal/index.ts";
  import { Button, Callout, Field, TextArea, TextInput } from "../../portal/ui/index.ts";
  import { COMMITTEES } from "../../portal/committees.ts";
  import { MESSAGE_MAX, expiryProblem, presetExpiry, sourceOrder } from "../format.ts";
  import { KINDS } from "../kinds.ts";
  import { approversOf, postsDirectly } from "./api.ts";
  import { actionNote, composerRole, submitLabel, type ComposerRole } from "./wording.ts";
  import ExpiryField from "./ExpiryField.svelte";
  import ComposerPreview from "./ComposerPreview.svelte";
  import ScheduleCard from "./ScheduleCard.svelte";

  let {
    initial = null,
    editing = null,
    live,
    now,
    onsave,
  }: {
    /** Starting values, for posting an expired announcement again. */
    initial?: Partial<AnnouncementInput> | null;
    /** The announcement being edited; null for a new one. */
    editing?: Announcement | null;
    /** The live tab, for the slots and the people who approve. */
    live: AnnouncementListResponse | null;
    /** The current time, kept fresh by the page. */
    now: Date;
    /** Saves the announcement. Throws an ApiError the form shows beside its fields. */
    onsave: (input: AnnouncementInput, role: ComposerRole, approvers: PersonRef[]) => Promise<void>;
  } = $props();

  const session = getPortalSession();
  const permissions = $derived(session.current.permissions);
  const sources = $derived(sourceOrder(permissions.tickerSources));

  // Fields start from the announcement being edited, the one posted again, or the defaults.
  function start<K extends keyof AnnouncementInput>(key: K, fallback: AnnouncementInput[K]): AnnouncementInput[K] {
    return editing?.[key] ?? initial?.[key] ?? fallback;
  }

  let kind = $state<AnnouncementKind>(start("kind", "event"));
  let committee = $state<TickerSource>(start("committee", sourceOrder(session.current.permissions.tickerSources)[0] ?? "club"));
  let message = $state(start("message", ""));
  let linkUrl = $state(start("linkUrl", null) ?? "");
  function startExpiry(): string {
    return editing?.expiresAt ?? presetExpiry("1w", now);
  }

  let expiresAt = $state<string | null>(startExpiry());
  let picking = $state(false);

  let errors = $state<Record<string, string>>({});
  let formError = $state<string | null>(null);
  let saving = $state(false);
  let messageInput = $state<HTMLTextAreaElement | null>(null);
  let linkInput = $state<HTMLInputElement | null>(null);
  let expiryField = $state<ExpiryField>();

  // A source the person may no longer post from stays selectable on its own announcement.
  const sourceChoices = $derived(sources.includes(committee) ? sources : [committee, ...sources]);
  const isLive = $derived(editing !== null && editing.state === "live");
  const isWaiting = $derived(editing !== null && (editing.state === "pending_approval" || editing.state === "sent_back"));
  const role = $derived(
    composerRole({
      isLive,
      isWaiting,
      manages: editing?.canManage === true,
      postsDirectly: postsDirectly(permissions, committee),
    }),
  );
  const direct = $derived(role === "edit-live" || role === "publish");
  const approvers = $derived(live === null ? [] : approversOf(live.approvers, committee));
  const slotsFull = $derived(role === "publish" && live !== null && live.slots.used >= live.slots.total);
  const note = $derived(actionNote(role, committeeName(committee), approvers.map((person) => person.name)));

  const draft = $derived({ kind, committee, message: message.trim() || "Your announcement", linkUrl: linkUrl.trim() || null });

  function onMessageInput() {
    // One line only: pasted line breaks become spaces.
    if (/[\r\n]/u.test(message)) message = message.replaceAll(/\s*[\r\n]+\s*/gu, " ");
    if (errors.message !== undefined) errors = { ...errors, message: "" };
  }

  function onMessageKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") event.preventDefault();
  }

  function check(): Record<string, string> {
    const problems: Record<string, string> = {};
    const text = message.trim();
    if (text === "") problems.message = "Add the message.";
    else if ([...text].length > MESSAGE_MAX) problems.message = `Keep the message to ${MESSAGE_MAX} characters or fewer.`;
    const link = linkUrl.trim();
    if (link !== "") {
      try {
        if (new URL(link).protocol !== "https:") problems.linkUrl = "Links must start with https://";
      } catch {
        problems.linkUrl = "Enter a full link starting with https://";
      }
    }
    const expiry = picking ? "Finish picking a date, or choose one of the options." : expiryProblem(expiresAt, new Date());
    if (expiry !== null) problems.expiresAt = expiry;
    return problems;
  }

  async function focusFirstProblem(problems: Record<string, string>) {
    await tick();
    if (problems.message) messageInput?.focus();
    else if (problems.linkUrl) linkInput?.focus();
    else if (problems.expiresAt) expiryField?.focus();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    formError = null;
    const problems = check();
    errors = problems;
    if (Object.keys(problems).length > 0 || expiresAt === null) {
      void focusFirstProblem(problems);
      return;
    }
    saving = true;
    try {
      await onsave(
        { kind, committee, message: message.trim(), linkUrl: linkUrl.trim() || null, expiresAt },
        role,
        approvers,
      );
    } catch (error) {
      if (isApiError(error) && Object.keys(error.fields).length > 0) {
        errors = { ...error.fields };
        void focusFirstProblem(errors);
      } else {
        formError = isApiError(error) ? error.message : "We could not save the announcement. Try again.";
      }
    } finally {
      saving = false;
    }
  }
</script>

<form class="composer" novalidate onsubmit={submit}>
  <div class="composer__preview">
    <ComposerPreview announcement={draft} {isLive} />
  </div>

  <div class="composer__card">
    <fieldset class="composer__group">
      <legend class="composer__legend">Type</legend>
      <div class="composer__pills">
        {#each KINDS as option (option.value)}
          <label class={["composer__pill", kind === option.value && "composer__pill--selected"]}>
            <input class="p-sr-only" type="radio" name="kind" value={option.value} bind:group={kind} />
            <option.icon size={16} strokeWidth={1.75} aria-hidden="true" />
            {option.label}
          </label>
        {/each}
      </div>
    </fieldset>

    <fieldset class="composer__group">
      <legend class="composer__legend">From</legend>
      <div class="composer__pills">
        {#each sourceChoices as source (source)}
          <label
            class={["composer__source", committee === source && "composer__source--selected"]}
            style:--source-color={COMMITTEES[source].color}
            style:--source-tint={COMMITTEES[source].tint}
          >
            <input class="p-sr-only" type="radio" name="committee" value={source} bind:group={committee} />
            <span class="composer__dot" aria-hidden="true"></span>
            {committeeName(source)}
          </label>
        {/each}
      </div>
      {#if errors.committee}<p class="composer__field-error" role="alert">{errors.committee}</p>{/if}
    </fieldset>

    <Field
      label="Message"
      help="One sentence. It scrolls, so shorter reads better."
      counter={{ length: [...message].length, max: MESSAGE_MAX }}
      error={errors.message}
      class="composer__message"
    >
      <TextArea
        bind:value={message}
        bind:ref={messageInput}
        rows={2}
        maxlength={MESSAGE_MAX + 20}
        oninput={onMessageInput}
        onkeydown={onMessageKeydown}
        placeholder="TartanHacks registration opens December 1."
      />
    </Field>

    <Field label="Link" optional error={errors.linkUrl} help="Where the announcement goes when someone clicks it.">
      <TextInput
        bind:value={linkUrl}
        bind:ref={linkInput}
        icon={Link}
        type="url"
        inputmode="url"
        autocomplete="url"
        placeholder="https://"
        oninput={() => {
          if (errors.linkUrl) errors = { ...errors, linkUrl: "" };
        }}
      />
    </Field>

    <ExpiryField bind:this={expiryField} bind:value={expiresAt} bind:picking {now} error={errors.expiresAt} />

    {#if formError !== null}
      <Callout tone="error" role="alert">{formError}</Callout>
    {:else if slotsFull}
      <Callout tone="warning">All {live?.slots.total} slots are in use. End an announcement first, then publish this one.</Callout>
    {/if}

    <div class="composer__actions">
      <Button variant="subtle" size="lg" href="/portal/ticker/" class="composer__cancel">Cancel</Button>
      <Button type="submit" size="lg" loading={saving}>{submitLabel(role)}</Button>
    </div>
    {#if note !== null}<p class="composer__note">{note}</p>{/if}
  </div>

  <div class="composer__schedule">
    <ScheduleCard
      {now}
      expiresAt={picking ? null : expiresAt}
      {editing}
      {direct}
      reviewing={role === "review"}
      {approvers}
      slots={live?.slots ?? null}
    />
  </div>
</form>

<style>
  .composer {
    display: grid;
    grid-template-columns: minmax(0, 660px) minmax(0, 1fr);
    grid-template-areas:
      "card preview"
      "card schedule"
      "card .";
    grid-template-rows: auto auto 1fr;
    gap: 20px 32px;
    align-items: start;
  }
  .composer__card {
    grid-area: card;
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 28px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .composer__preview {
    grid-area: preview;
  }
  .composer__schedule {
    grid-area: schedule;
  }
  .composer__group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .composer__legend {
    margin-bottom: 10px;
    padding: 0;
    color: var(--p-text);
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 16px;
  }
  .composer__pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .composer__pill {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 14px 0 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-sm);
    background: var(--p-surface);
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    cursor: pointer;
    transition: background-color var(--p-duration) var(--p-ease);

    &:hover:not(.composer__pill--selected) {
      background: var(--p-surface-muted);
    }
    &:has(:focus-visible) {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .composer__pill--selected {
    border-color: var(--p-inverse);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
  }
  .composer__source {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 11px 0 10px;
    border: 1.5px solid transparent;
    border-radius: var(--p-radius-pill);
    background: var(--source-tint);
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 14px;
    cursor: pointer;

    &:hover:not(.composer__source--selected) {
      border-color: var(--p-border);
    }
    &:has(:focus-visible) {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .composer__source--selected {
    border-color: var(--p-text);
  }
  .composer__dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--source-color);
  }
  .composer__field-error {
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .composer :global(.composer__message textarea) {
    min-height: 72px;
    padding: 12px 14px;
    font-size: 1rem;
    line-height: 24px;
    resize: none;
  }
  .composer__actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
  }
  .composer__note {
    margin: -8px 0 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }

  @media (max-width: 960px) {
    .composer {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas:
        "preview"
        "card"
        "schedule";
      grid-template-rows: auto;
    }
  }
  /* Phones, as in the mobile design: the fields straight on the page, edge to edge past the
     shell's 16 px padding, and the main button in a bar that stays above the tab bar. The app bar
     has Cancel, and the expiry sentence says when it leaves, so the schedule card is left out. */
  @media (max-width: 600px) {
    .composer {
      gap: 0;
    }
    .composer__card {
      gap: 20px;
      margin-inline: -16px;
      padding: 20px 20px 0;
      border: 0;
      border-radius: 0;
    }
    .composer__actions {
      position: sticky;
      /* Tucked a pixel under the tab bar, so nothing scrolls through a gap between them. */
      bottom: calc(var(--p-tabbar-height) + env(safe-area-inset-bottom) - 2px);
      z-index: 5;
      order: 1;
      margin-inline: -20px;
      padding: 14px 20px 18px;
      border-top: 1px solid var(--p-border);
      background: var(--p-surface);
    }
    .composer__actions :global(.p-button) {
      width: 100%;
    }
    .composer__actions :global(.composer__cancel) {
      display: none;
    }
    .composer__note {
      margin: 0;
    }
    .composer__schedule {
      display: none;
    }
  }
</style>
