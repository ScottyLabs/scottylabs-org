<script lang="ts">
  // The fields of a project's draft (Figma 109:6800; spec PR-2, PR-3): basics, description,
  // photos and video, links and "Built with". Edits go straight into `doc`; the page saves them.
  import type { CommitteeSlug, ProjectDoc, ProjectKind, ProjectStatus } from "../../api/types.ts";
  import RichTextEditor from "../../editor/RichTextEditor.svelte";
  import ImageListField from "../../media/ImageListField.svelte";
  import { COMMITTEE_LIST } from "../../portal/committees.ts";
  import { CommitteeChip, Field, Select, TextInput } from "../../portal/ui/index.ts";
  import {
    MAX_DESCRIPTION_CHARS,
    MAX_NAME_CHARS,
    MAX_PHOTOS,
    MAX_SUMMARY_CHARS,
  } from "../drafts.ts";
  import { PROJECT_KIND_LABELS, PROJECT_STATUS_LABELS } from "../labels.ts";

  let {
    doc = $bindable(),
    errors,
    disabled = false,
    committees,
  }: {
    doc: ProjectDoc;
    /** Field problems from the server, keyed by field path. */
    errors: Readonly<Record<string, string>>;
    disabled?: boolean;
    /** Committees this person may move the project to. The current one is always listed. */
    committees: readonly CommitteeSlug[];
  } = $props();

  const committeeOptions = $derived(
    COMMITTEE_LIST.filter(
      (style) => style.slug === doc.committee || committees.includes(style.slug as CommitteeSlug),
    ).map((style) => ({ value: style.slug as CommitteeSlug, label: style.name })),
  );
  const statusOptions = (Object.keys(PROJECT_STATUS_LABELS) as ProjectStatus[]).map((value) => ({
    value,
    label: PROJECT_STATUS_LABELS[value],
  }));
  const kindOptions = (Object.keys(PROJECT_KIND_LABELS) as ProjectKind[]).map((value) => ({
    value,
    label: PROJECT_KIND_LABELS[value],
  }));
  const otherCommittees = $derived(COMMITTEE_LIST.filter((style) => style.slug !== doc.committee));
  const photoErrors = $derived(
    doc.media.map((_, index) => errors[`media.${index.toString()}.alt`] ?? errors[`media.${index.toString()}`]),
  );

  function toggleBuiltWith(committee: CommitteeSlug): void {
    doc.builtWith = doc.builtWith.includes(committee)
      ? doc.builtWith.filter((slug) => slug !== committee)
      : [...doc.builtWith, committee];
  }

  function setLink(key: "siteUrl" | "repoUrl" | "videoUrl", value: string): void {
    doc[key] = value.trim() === "" ? null : value;
  }
</script>

<div class="form">
  <section class="section" aria-labelledby="basics-heading">
    <h2 class="section__heading" id="basics-heading">Basics</h2>
    <div class="grid">
      <Field
        label="Project name"
        error={errors.name}
        counter={{ length: doc.name.trim().length, max: MAX_NAME_CHARS }}
        required
      >
        <TextInput bind:value={doc.name} {disabled} autocomplete="off" />
      </Field>
      <Field
        label="Committee"
        error={errors.committee}
        help={committeeOptions.length < 2 ? "Only an admin can move it to another committee." : undefined}
      >
        <Select
          bind:value={doc.committee}
          options={committeeOptions}
          disabled={disabled || committeeOptions.length < 2}
        />
      </Field>
    </div>
    <Field
      label="One-line summary"
      help="Shown on the project card and under the name on its page."
      error={errors.summary}
      counter={{ length: doc.summary.trim().length, max: MAX_SUMMARY_CHARS }}
    >
      <TextInput bind:value={doc.summary} {disabled} autocomplete="off" />
    </Field>
    <div class="grid">
      <Field label="Status" error={errors.status}>
        <Select bind:value={doc.status} options={statusOptions} {disabled} />
      </Field>
      <Field label="Type" error={errors.kind}>
        <Select bind:value={doc.kind} options={kindOptions} {disabled} />
      </Field>
    </div>
  </section>

  <section class="section" aria-labelledby="description-heading">
    <div class="section__intro">
      <h2 class="section__heading" id="description-heading">Description</h2>
      <p class="section__text">A few sentences on what it is and who it is for.</p>
    </div>
    <Field label="Description" hideLabel error={errors.description}>
      <RichTextEditor
        bind:value={doc.description}
        maxChars={MAX_DESCRIPTION_CHARS}
        placeholder="What it does, who it is for and how it came to be."
        {disabled}
      />
    </Field>
  </section>

  <section class="section" aria-labelledby="photos-heading">
    <div class="section__intro">
      <h2 class="section__heading" id="photos-heading">Photos and video</h2>
      <p class="section__text">Up to {MAX_PHOTOS} photos. The first one is the cover and needs a description.</p>
    </div>
    <ImageListField
      bind:value={doc.media}
      max={MAX_PHOTOS}
      altRequired="first"
      error={errors.media}
      altErrors={photoErrors}
      {disabled}
    />
    <Field
      label="Video link"
      optional
      help="A YouTube or Vimeo link. Visitors load the video only when they press play."
      error={errors.videoUrl}
    >
      <TextInput
        value={doc.videoUrl ?? ""}
        oninput={(event) => setLink("videoUrl", event.currentTarget.value)}
        type="url"
        inputmode="url"
        placeholder="Paste a YouTube or Vimeo link"
        {disabled}
      />
    </Field>
  </section>

  <section class="section" aria-labelledby="links-heading">
    <h2 class="section__heading" id="links-heading">Links</h2>
    <Field label="Live site" optional error={errors.siteUrl}>
      <TextInput
        value={doc.siteUrl ?? ""}
        oninput={(event) => setLink("siteUrl", event.currentTarget.value)}
        type="url"
        inputmode="url"
        placeholder="https://"
        {disabled}
      />
    </Field>
    <Field label="Repository" optional help="Shown under the description." error={errors.repoUrl}>
      <TextInput
        value={doc.repoUrl ?? ""}
        oninput={(event) => setLink("repoUrl", event.currentTarget.value)}
        type="url"
        inputmode="url"
        placeholder="https://git.cmu.dev/ScottyLabs/"
        {disabled}
      />
    </Field>
  </section>

  <section class="section" aria-labelledby="built-heading">
    <div class="section__intro">
      <h2 class="section__heading" id="built-heading">Built with</h2>
      <p class="section__text" id="built-help">
        Other committees that worked on this. Each links to its committee page.
      </p>
    </div>
    <div class="chips" role="group" aria-labelledby="built-heading" aria-describedby="built-help">
      {#each otherCommittees as style (style.slug)}
        <CommitteeChip
          committee={style.slug}
          selectable
          selected={doc.builtWith.includes(style.slug as CommitteeSlug)}
          {disabled}
          onclick={() => toggleBuiltWith(style.slug as CommitteeSlug)}
        />
      {/each}
    </div>
  </section>
</div>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 44px;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .section__intro {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .section__heading {
    margin: 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .section__text {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 18px 16px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  @media (max-width: 600px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
