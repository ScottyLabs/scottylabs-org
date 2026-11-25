<script lang="ts">
  // Publish (Figma 109:9546, spec CP-9 and CP-11): saves what is waiting, lists the changes since
  // the last published version, and lists anything that blocks publishing, such as a photo
  // without alt text, with a way to go to it.
  import Check from "@lucide/svelte/icons/check";
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import { isAbortError, isApiError } from "../../api/client.ts";
  import { getPortalSession, getToasts } from "../../portal/index.ts";
  import { Button, Callout, Dialog, Skeleton } from "../../portal/ui/index.ts";
  import { loadChanges, publishPage } from "../api.ts";
  import { problemsFor } from "../errors.ts";
  import { committeePath, displayPath } from "../links.ts";
  import { type PublishProblem, publishProblems } from "./publish-checks.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    open = $bindable(false),
    onshow,
  }: {
    editor: PageEditor;
    open?: boolean;
    /** Selects the block with a problem, closing the dialog. */
    onshow: (blockId: string | null) => void;
  } = $props();

  const session = getPortalSession();
  const toasts = getToasts();

  let changes = $state<string[] | null>(null);
  let problems = $state<PublishProblem[]>([]);
  let failure = $state<string | null>(null);
  let preparing = $state(false);
  let publishing = $state(false);

  // Each time it opens: save first, then check the page and load what changed.
  $effect(() => {
    if (!open) return;
    const controller = new AbortController();
    void prepare(controller.signal);
    return () => controller.abort();
  });

  async function prepare(signal: AbortSignal) {
    changes = null;
    failure = null;
    preparing = true;
    const saved = await editor.flush();
    if (signal.aborted) return;
    if (!saved) {
      preparing = false;
      failure = editor.saveMessage ?? "Your latest changes are not saved yet, so they cannot be published. Try again in a moment.";
      problems = [];
      return;
    }
    problems = publishProblems(editor.doc);
    try {
      changes = (await loadChanges(editor.slug, signal)).changes;
    } catch (error) {
      if (isAbortError(error)) return;
      failure = isApiError(error) ? error.message : "We could not load the changes. Try again.";
    } finally {
      preparing = false;
    }
  }

  async function publish() {
    publishing = true;
    failure = null;
    try {
      const response = await publishPage(editor.slug, editor.version);
      editor.replace(response);
      open = false;
      toasts.success(`Published. The ${editor.committee.name} page is live.`, {
        label: "View page",
        href: committeePath(editor.slug),
        external: true,
      });
    } catch (error) {
      if (!isApiError(error)) throw error;
      failure = error.message;
      if (error.status === 422) {
        editor.problems = problemsFor(editor.doc, error.fields);
      }
    } finally {
      publishing = false;
    }
  }

  const name = $derived(session.me?.user.name ?? "");
  const blocked = $derived(problems.length > 0);
  const nothingNew = $derived(changes !== null && changes.length === 0 && editor.status === "published");
</script>

<Dialog
  bind:open
  title={`Publish the ${editor.committee.name} page?`}
  description={`Your changes go live at ${displayPath(editor.slug)} right away. You can keep editing after you publish.`}
  dismissible={!publishing}
>
  {#if blocked}
    <div class="publish__box publish__box--problems" role="alert">
      <p class="publish__count">
        {problems.length === 1 ? "Fix 1 thing before you publish" : `Fix ${problems.length} things before you publish`}
      </p>
      <ul class="publish__list">
        {#each problems as problem, index (index)}
          <li>
            <CircleAlert size={16} strokeWidth={2} aria-hidden="true" />
            <span class="publish__text">{problem.message}</span>
            <button
              type="button"
              class="publish__show"
              onclick={() => {
                open = false;
                onshow(problem.blockId);
              }}
            >
              Show me
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {:else if preparing || changes === null}
    {#if failure === null}
      <div class="publish__box" aria-busy="true">
        <p class="p-sr-only" role="status">Saving and checking the page.</p>
        <Skeleton width="40%" height="16px" />
        <Skeleton width="80%" height="18px" />
        <Skeleton width="70%" height="18px" />
      </div>
    {/if}
  {:else if nothingNew}
    <div class="publish__box">
      <p class="publish__count">Nothing has changed since the last version.</p>
    </div>
  {:else}
    <div class="publish__box">
      <p class="publish__count">
        {changes.length === 1 ? "1 change since the last version" : `${changes.length} changes since the last version`}
      </p>
      <ul class="publish__list">
        {#each changes as change, index (index)}
          <li><Check size={16} strokeWidth={2} aria-hidden="true" /><span class="publish__text">{change}</span></li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if failure !== null}
    <Callout tone="error" role="alert">{failure}</Callout>
  {/if}

  {#snippet footer()}
    {#if name !== ""}<p class="publish__who">Publishing as {name}</p>{/if}
    <Button variant="outline" disabled={publishing} onclick={() => (open = false)}>Keep editing</Button>
    <Button loading={publishing} disabled={blocked || preparing || changes === null || nothingNew} onclick={publish}>
      Publish now
    </Button>
  {/snippet}
</Dialog>

<style>
  .publish__box {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
  }
  .publish__box--problems {
    background: var(--p-danger-surface);
  }
  .publish__count {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .publish__list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;

    & li {
      display: flex;
      align-items: flex-start;
      gap: 10px;
      color: var(--p-text);
      font-size: 0.9375rem;
      line-height: 22px;
    }
    & li > :global(svg) {
      flex: none;
      margin-top: 3px;
    }
  }
  .publish__box--problems .publish__list li > :global(svg) {
    color: var(--p-danger-text);
  }
  .publish__text {
    flex: 1;
  }
  .publish__show {
    flex: none;
    color: var(--p-info-text);
    font-size: 0.875rem;
    text-decoration: underline;
    cursor: pointer;

    &:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .publish__who {
    margin: 0 auto 0 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
</style>
