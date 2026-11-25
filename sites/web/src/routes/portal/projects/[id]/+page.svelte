<script lang="ts">
  // The project editor (Figma 109:6800, earlier 44:7922; phone 112:10399; spec PR-2 to PR-9).
  // The draft saves itself a moment after each change. Admins and the owning committee's
  // directors publish; leads send it for approval. Visitors keep seeing the published version
  // until the draft is published.
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { beforeNavigate, goto } from "$app/navigation";
  import { page } from "$app/state";
  import Archive from "@lucide/svelte/icons/archive";
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import Eye from "@lucide/svelte/icons/eye";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Send from "@lucide/svelte/icons/send";
  import { type ApiError, get, isApiError, post, put } from "#lib/api/client.ts";
  import type { ProjectDoc, ProjectEditorResponse } from "#lib/api/types.ts";
  import { COMMITTEES } from "#lib/portal/committees.ts";
  import { formatUpdated } from "#lib/portal/dates.ts";
  import PortalError from "#lib/portal/PortalError.svelte";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import { getToasts } from "#lib/portal/toasts.svelte.ts";
  import {
    Badge,
    Button,
    Callout,
    Dialog,
    Menu,
    type MenuItem,
    Skeleton,
  } from "#lib/portal/ui/index.ts";
  import { draftCard, draftDetail, stateLabel } from "#lib/projects/drafts.ts";
  import ApprovalNotice from "#lib/projects/editor/ApprovalNotice.svelte";
  import { DraftSaver } from "#lib/projects/editor/draft-saver.svelte.ts";
  import LivePreview from "#lib/projects/editor/LivePreview.svelte";
  import ProjectEditors from "#lib/projects/editor/ProjectEditors.svelte";
  import ProjectForm from "#lib/projects/editor/ProjectForm.svelte";
  import ProjectSettings from "#lib/projects/editor/ProjectSettings.svelte";
  import PublishChecklist from "#lib/projects/editor/PublishChecklist.svelte";
  import { projectPath } from "#lib/projects/labels.ts";

  type Project = ProjectEditorResponse["project"];

  const session = getPortalSession();
  const toasts = getToasts();

  const id = $derived(page.params.id ?? "");
  let server = $state.raw<Project | null>(null);
  let doc = $state<ProjectDoc | null>(null);
  let saver = $state.raw<DraftSaver | null>(null);
  let loadError = $state.raw<ApiError | null>(null);
  let publishErrors = $state<Record<string, string>>({});
  let actionError = $state<string | null>(null);
  let busy = $state<"publish" | "discard" | "archive" | "unarchive" | "preview" | null>(null);
  let sendOpen = $state(false);
  let discardOpen = $state(false);
  let archiveOpen = $state(false);
  let now = $state(new Date());

  const canEdit = $derived(server?.canEdit ?? false);
  const isAdmin = $derived(session.permissions?.isAdmin ?? false);
  const errors = $derived({ ...publishErrors, ...saver?.errors });
  const name = $derived(doc === null || doc.name.trim() === "" ? "This project" : doc.name.trim());
  const meta = $derived(
    server === null ? null : { id: server.id, slug: server.slug, featured: server.featured },
  );
  const card = $derived(doc === null || meta === null ? null : draftCard(doc, meta));
  const detail = $derived(doc === null || meta === null ? null : draftDetail(doc, meta));
  const dirty = $derived(saver !== null && saver.status !== "saved");
  const waiting = $derived(server?.pendingApproval?.status === "waiting");
  const approverNames = $derived(
    server === null || server.approvers.length === 0
      ? "a director"
      : server.approvers.map((person) => person.name).join(" or "),
  );
  const status = $derived(server === null ? null : stateLabel(server));
  const canDiscard = $derived(
    canEdit && server !== null && server.published !== null && (server.hasUnpublishedChanges || dirty),
  );
  const upToDate = $derived(
    server !== null && server.state === "published" && !server.hasUnpublishedChanges && !dirty,
  );
  const publishLabel = $derived(
    server?.state === "published" ? (upToDate ? "Published" : "Publish changes") : "Publish",
  );
  const saveLabel = $derived(describeSave());
  // Only saves made on this visit are announced, not the time of the last one before it.
  let savedHere = $state(false);
  const saveAnnouncement = $derived.by(() => {
    if (saver === null) return "";
    if (saver.status === "invalid" || (saver.status === "failed" && saver.retrying)) return saveLabel?.text ?? "";
    return saver.status === "saved" && savedHere ? "Saved" : "";
  });
  $effect(() => {
    if (saver !== null && (saver.status === "pending" || saver.status === "saving")) savedHere = true;
  });

  const menuItems = $derived.by((): MenuItem[] => {
    if (server === null) return [];
    const items: MenuItem[] = [];
    if (server.state === "published") {
      items.push({ label: "View public page", icon: ArrowUpRight, href: projectPath(server.slug), external: true });
    }
    if (canDiscard) items.push({ label: "Discard changes", icon: RotateCcw, onSelect: () => (discardOpen = true) });
    if (server.canArchive && server.state !== "archived") {
      items.push({ label: "Archive project", icon: Archive, danger: true, onSelect: () => (archiveOpen = true) });
    }
    return items;
  });

  $effect(() => {
    const current = id;
    untrack(() => void load(current));
  });

  function onunload(event: BeforeUnloadEvent): void {
    if (saver?.dirty) event.preventDefault();
  }

  onMount(() => {
    const ticker = setInterval(() => (now = new Date()), 30_000);
    addEventListener("beforeunload", onunload);
    return () => {
      clearInterval(ticker);
      removeEventListener("beforeunload", onunload);
    };
  });

  onDestroy(() => saver?.destroy());

  // Leaving with a save on its way sends it right away. Leaving with changes the server refused
  // asks first, since they would be lost.
  beforeNavigate(({ cancel, willUnload }) => {
    if (saver === null || willUnload || !saver.dirty) return;
    if (saver.status === "invalid" || saver.status === "failed") {
      if (!confirm("Your latest changes are not saved. Leave this page anyway?")) cancel();
      return;
    }
    void saver.flush();
  });

  // Every edit goes to the saver; it saves a moment after the last one. What the last action
  // reported is about the project before this edit, so it goes too.
  $effect(() => {
    if (doc === null) return;
    const json = JSON.stringify(doc);
    untrack(() => {
      if (saver === null) return;
      publishErrors = {};
      actionError = null;
      saver.change(JSON.parse(json) as ProjectDoc);
    });
  });

  async function load(current: string): Promise<void> {
    saver?.destroy();
    saver = null;
    server = null;
    doc = null;
    loadError = null;
    try {
      const response = await get<ProjectEditorResponse>(`/projects/${encodeURIComponent(current)}`);
      if (current !== id) return;
      startEditing(response.project);
    } catch (error) {
      if (isApiError(error)) loadError = error;
    }
  }

  /** Shows the server's draft in the form and saves changes to it from here on. */
  function startEditing(project: Project): void {
    saver?.destroy();
    savedHere = false;
    server = project;
    doc = structuredClone(project.draft);
    saver = project.canEdit
      ? new DraftSaver({
          initial: structuredClone(project.draft),
          savedAt: new Date(project.updatedAt),
          save: (draft) => put<ProjectEditorResponse>(`/projects/${project.id}/draft`, { doc: draft }),
          onsaved: (response) => (server = response.project),
        })
      : null;
  }

  function describeSave(): { text: string; tone: "quiet" | "warning" | "danger" } | null {
    if (saver === null) return null;
    if (saver.status === "pending" || saver.status === "saving") return { text: "Saving\u2026", tone: "quiet" };
    if (saver.status === "invalid") return { text: "Not saved. Fix the highlighted fields.", tone: "danger" };
    if (saver.status === "failed") {
      return saver.retrying
        ? { text: "Not saved yet. Trying again.", tone: "warning" }
        : { text: "Not saved", tone: "danger" };
    }
    if (saver.savedAt === null) return null;
    // The clock ticks every 30 seconds, so a save can be newer than it.
    const when = formatUpdated(saver.savedAt, saver.savedAt > now ? saver.savedAt : now);
    return { text: when === "Just now" ? "Saved just now" : `Saved ${when}`, tone: "quiet" };
  }

  /** Waits for the draft to reach the server. Says what is wrong when it cannot. */
  async function saveFirst(action: string): Promise<boolean> {
    if (saver === null) return true;
    if (await saver.flush()) return true;
    actionError =
      saver.status === "invalid"
        ? `Fix the highlighted fields, then ${action}.`
        : (saver.message ?? "We couldn\u2019t save your changes. Try again in a moment.");
    return false;
  }

  function failed(error: unknown, fallback: string): void {
    if (isApiError(error)) {
      publishErrors = { ...error.fields };
      actionError = error.message;
    } else {
      actionError = fallback;
    }
  }

  async function publish(): Promise<void> {
    if (server === null) return;
    actionError = null;
    busy = "publish";
    try {
      if (!(await saveFirst(server.canPublish ? "publish" : "send it"))) return;
      const response = await post<ProjectEditorResponse>(`/projects/${server.id}/publish`);
      if (response.project.canPublish) {
        saver?.destroy();
        toasts.success(`${response.project.draft.name} is published.`, {
          label: "View page",
          href: projectPath(response.project.slug),
          external: true,
        });
        await goto("/portal/projects/");
        return;
      }
      server = response.project;
      sendOpen = false;
      toasts.success(`Sent to ${approverNames} for approval.`);
    } catch (error) {
      sendOpen = false;
      failed(error, "We couldn\u2019t publish it. Try again.");
    } finally {
      busy = null;
    }
  }

  /** After an action whose control went away, start again from the page heading. */
  async function focusHeading(): Promise<void> {
    await tick();
    document.querySelector<HTMLElement>("#portal-main h1")?.focus();
  }

  async function discard(): Promise<void> {
    if (server === null) return;
    actionError = null;
    busy = "discard";
    saver?.destroy();
    try {
      const response = await post<ProjectEditorResponse>(`/projects/${server.id}/discard`);
      startEditing(response.project);
      discardOpen = false;
      toasts.success("Your changes are discarded. The draft matches the published page again.");
      void focusHeading();
    } catch (error) {
      discardOpen = false;
      failed(error, "We couldn\u2019t discard the changes. Try again.");
    } finally {
      busy = null;
    }
  }

  async function setArchived(archived: boolean): Promise<void> {
    if (server === null) return;
    actionError = null;
    busy = archived ? "archive" : "unarchive";
    try {
      const response = await post<ProjectEditorResponse>(
        `/projects/${server.id}/${archived ? "archive" : "unarchive"}`,
      );
      server = response.project;
      archiveOpen = false;
      void focusHeading();
      toasts.success(
        archived
          ? `${name} is archived. It no longer appears on the site.`
          : response.project.state === "published"
            ? `${name} is back on the Projects page.`
            : `${name} is a draft again.`,
      );
    } catch (error) {
      archiveOpen = false;
      failed(error, "We couldn\u2019t change that. Try again.");
    } finally {
      busy = null;
    }
  }

  async function preview(): Promise<void> {
    if (server === null) return;
    actionError = null;
    busy = "preview";
    try {
      if (await saveFirst("preview the page")) await goto(`/portal/projects/${server.id}/preview/`);
    } finally {
      busy = null;
    }
  }

  function startPublish(): void {
    if (server?.canPublish) void publish();
    else sendOpen = true;
  }
</script>

<svelte:head>
  <title>{server === null ? "Project" : `Edit ${name}`} - ScottyLabs Portal</title>
</svelte:head>

{#snippet primaryAction(fullWidth: boolean)}
  {#if server !== null && server.state === "archived" && server.canArchive}
    <Button {fullWidth} size="lg" loading={busy === "unarchive"} onclick={() => void setArchived(false)}>
      Unarchive
    </Button>
  {:else if server !== null && server.canPublish}
    <Button {fullWidth} size="lg" loading={busy === "publish"} disabled={upToDate || busy !== null} onclick={startPublish}>
      {publishLabel}
    </Button>
  {:else if server !== null && canEdit}
    <Button
      {fullWidth}
      size="lg"
      icon={Send}
      loading={busy === "publish"}
      disabled={(waiting && !dirty) || server.state === "archived" || busy !== null}
      onclick={startPublish}
    >
      {waiting && !dirty ? "Waiting for approval" : "Send for approval"}
    </Button>
  {/if}
{/snippet}

{#if loadError !== null}
  <PortalError status={loadError.status} message={loadError.message} />
{:else if server === null || doc === null}
  <div class="loading" aria-busy="true">
    <p class="p-sr-only" role="status">Loading the project</p>
    <Skeleton width="160px" height="14px" />
    <Skeleton width="320px" height="44px" />
    <div class="loading__body">
      <div class="loading__form">
        {#each [0, 1, 2, 3] as row (row)}
          <Skeleton width="100%" height="64px" radius="10px" />
        {/each}
      </div>
      <Skeleton width="100%" height="360px" radius="20px" />
    </div>
  </div>
{:else}
  <nav class="crumbs" aria-label="Breadcrumb">
    <ol>
      <li><a href="/portal/projects/">Projects</a></li>
      <li><span aria-current="page">{name}</span></li>
    </ol>
  </nav>

  <!-- On phones this is the app bar of Figma 112:10399: back, the title and the status. -->
  <header class="header">
    <div class="header__title">
      <a class="header__back" href="/portal/projects/">
        <ChevronLeft size={20} strokeWidth={2} aria-hidden="true" />
        <span class="p-sr-only">Back to projects</span>
      </a>
      <h1 class="title" tabindex="-1">{canEdit ? "Edit project" : name}</h1>
      {#if status !== null}<Badge tone={status.tone === "success" ? "success" : status.tone === "warning" ? "warning" : "neutral"}>{status.label}</Badge>{/if}
    </div>
    <div class="header__actions">
      {#if saveLabel !== null}
        <span class={["save", `save--${saveLabel.tone}`]}>{saveLabel.text}</span>
      {/if}
      <!-- A save that stopped for good is announced by its message below. "Saving" stays quiet;
           each finished save says "Saved" once, since the text empties while the next one runs. -->
      <span class="p-sr-only" aria-live="polite">{saveAnnouncement}</span>
      <span class="header__desktop">
        <Button variant="outline" size="lg" icon={Eye} loading={busy === "preview"} onclick={() => void preview()}>
          Preview page
        </Button>
        {@render primaryAction(false)}
      </span>
      {#if menuItems.length > 0}
        <Menu items={menuItems} label="More actions">
          {#snippet trigger(props)}
            <button {...props} type="button" class="more" aria-label="More actions">
              <Ellipsis size={20} strokeWidth={2} aria-hidden="true" />
            </button>
          {/snippet}
        </Menu>
      {/if}
    </div>
  </header>
  {#if saveLabel !== null}
    <p class={["save", "save-line", `save--${saveLabel.tone}`]} aria-hidden="true">{saveLabel.text}</p>
  {/if}

  <div class="notices">
    {#if !canEdit}
      <Callout tone="info">
        You can view this project. Admins, the director of {COMMITTEES[server.draft.committee].name}
        and its editors can change it.
      </Callout>
    {/if}
    {#if server.pendingApproval !== null}
      <ApprovalNotice approval={server.pendingApproval} approvers={server.approvers} isLead={canEdit && !server.canPublish} />
    {/if}
    {#if server.state === "archived"}
      <Callout tone="warning" title="Archived">
        Visitors can&rsquo;t see it. {server.canArchive ? "Unarchive it to show it on the site again." : ""}
      </Callout>
    {/if}
    {#if actionError !== null}
      <Callout tone="error" role="alert">{actionError}</Callout>
    {:else if saver !== null && saver.status === "failed" && !saver.retrying && saver.message !== null}
      <Callout tone="error" role="alert">{saver.message}</Callout>
    {/if}
  </div>

  <div class="layout">
    <div class="layout__main">
      <ProjectForm
        bind:doc
        {errors}
        disabled={!canEdit}
        committees={server.canPublish ? (session.permissions?.projectCommittees ?? []) : []}
      />

      {#if isAdmin}
        <div class="extra">
          <ProjectSettings
            projectId={server.id}
            {name}
            slug={server.slug}
            featured={server.featured}
            onchange={(response) => (server = response.project)}
          />
        </div>
      {/if}
      {#if server.canManageEditors}
        <div class="extra">
          <ProjectEditors
            projectId={server.id}
            projectName={name}
            committee={server.draft.committee}
            editors={server.editors}
            onchange={(response) => (server = response.project)}
          />
        </div>
      {/if}
    </div>

    <aside class="layout__side" aria-label="Preview and checklist">
      {#if card !== null && detail !== null}
        <LivePreview {card} {detail} />
      {/if}
      <PublishChecklist
        checklist={server.checklist}
        needsApproval={!server.canPublish}
        approvers={server.approvers}
      />
    </aside>
  </div>

  <div class="mobile-bar">
    <Button variant="outline" size="lg" fullWidth loading={busy === "preview"} onclick={() => void preview()}>
      Preview
    </Button>
    {@render primaryAction(true)}
  </div>
{/if}

<Dialog
  bind:open={sendOpen}
  title={`Send ${name} for approval?`}
  description={`${approverNames} reviews it before it goes live. Visitors keep seeing the published version until then.`}
  dismissible={busy !== "publish"}
>
  {#snippet footer()}
    <Button variant="outline" disabled={busy === "publish"} onclick={() => (sendOpen = false)}>Keep editing</Button>
    <Button icon={Send} loading={busy === "publish"} onclick={() => void publish()}>Send for approval</Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={discardOpen}
  title={`Discard your changes to ${name}?`}
  description="The draft goes back to what is published now. This can't be undone."
  dismissible={busy !== "discard"}
>
  {#snippet footer()}
    <Button variant="outline" disabled={busy === "discard"} onclick={() => (discardOpen = false)}>Keep editing</Button>
    <Button variant="danger" loading={busy === "discard"} onclick={() => void discard()}>Discard changes</Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={archiveOpen}
  title={`Archive ${name}?`}
  description="It leaves the Projects page and its address stops working. Nothing is deleted, and you can unarchive it at any time."
  dismissible={busy !== "archive"}
>
  {#snippet footer()}
    <Button variant="outline" disabled={busy === "archive"} onclick={() => (archiveOpen = false)}>Keep it</Button>
    <Button variant="danger" loading={busy === "archive"} onclick={() => void setArchived(true)}>Archive project</Button>
  {/snippet}
</Dialog>

<style>
  .crumbs ol {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 0.875rem;
    line-height: 20px;
  }
  .crumbs li {
    display: inline-flex;
    gap: 8px;
    color: var(--p-text-secondary);

    &:not(:last-child)::after {
      content: "/";
    }
  }
  .crumbs a {
    color: var(--p-text-secondary);
    border-radius: 4px;

    &:hover {
      color: var(--p-text);
      text-decoration: underline;
      text-underline-offset: 3px;
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .crumbs [aria-current="page"] {
    color: var(--p-text);
  }

  .header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 16px 24px;
    margin-top: 10px;
  }
  .header__title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .title {
    margin: 0;
    font-size: 2.75rem;
    font-weight: 700;
    line-height: 1.14;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;

    &:focus {
      outline: none;
    }
  }
  .header__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .header__desktop {
    display: contents;
  }
  .save {
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .save--quiet {
    color: var(--p-text-tertiary);
  }
  .save--warning {
    color: var(--p-warning-text);
  }
  .save--danger {
    color: var(--p-danger-text);
  }
  .more {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    box-sizing: border-box;
    border: 1px solid var(--p-border);
    border-radius: 50%;
    background: var(--p-surface);
    color: var(--p-text);
    cursor: pointer;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }

  .notices {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 20px;

    &:empty {
      display: none;
    }
  }

  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, 400px);
    gap: 64px;
    align-items: start;
    margin-top: 36px;
  }
  .layout__main {
    min-width: 0;
  }
  .layout__side {
    position: sticky;
    top: calc(var(--p-topbar-height) + 24px);
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .extra {
    margin-top: 44px;
    padding-top: 36px;
    border-top: 1px solid var(--p-border);
  }

  .loading {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .loading__body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, 400px);
    gap: 64px;
    margin-top: 36px;
  }
  .loading__form {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .mobile-bar {
    display: none;
  }
  .header__back,
  .save-line {
    display: none;
  }

  @media (max-width: 1100px) {
    .layout,
    .loading__body {
      grid-template-columns: minmax(0, 1fr);
      gap: 44px;
    }
    .layout__side {
      position: static;
    }
  }
  @media (max-width: 899px) {
    .title {
      font-size: 2rem;
    }
    .header__desktop {
      display: none;
    }
    .mobile-bar {
      position: sticky;
      bottom: calc(var(--p-tabbar-height) + env(safe-area-inset-bottom));
      z-index: 5;
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 10px;
      margin: 32px -16px 0;
      padding: 12px 16px;
      border-top: 1px solid var(--p-border);
      background: var(--p-surface);
    }
  }
  /* The portal's bars are hidden here on phones (see the portal layout), so the header is the
     app bar, edge to edge past the main area's 20 px and 16 px padding. */
  @media (max-width: 600px) {
    .crumbs {
      display: none;
    }
    .header {
      position: sticky;
      top: 0;
      z-index: 6;
      flex-wrap: nowrap;
      gap: 8px;
      box-sizing: border-box;
      height: 56px;
      margin: -20px -16px 0;
      padding: 0 8px 0 4px;
      border-bottom: 1px solid var(--p-border);
      background: var(--p-surface);
    }
    .header__title {
      flex-wrap: nowrap;
      gap: 6px;
    }
    .header__back {
      display: inline-grid;
      flex: none;
      place-items: center;
      width: 40px;
      height: 40px;
      border-radius: var(--p-radius-sm);
      color: var(--p-text);

      &:focus-visible {
        outline: 2px solid var(--p-focus);
        outline-offset: -2px;
      }
    }
    .title {
      overflow: hidden;
      font-size: 1rem;
      font-weight: 500;
      line-height: 24px;
      letter-spacing: 0;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .header__actions .save {
      display: none;
    }
    .more {
      width: 40px;
      height: 40px;
      border: 0;
    }
    .save-line {
      display: block;
      margin: 12px 0 0;
    }
  }
</style>
