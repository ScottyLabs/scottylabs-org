<script lang="ts">
  // Settings for the selected block (Figma 109:7297, 109:8129, 109:8655): its width, the fields
  // that are easier to change here than on the page, the theme rows that cannot change, and
  // Remove block. `compact` leaves out the width, for the phone editor.
  import type { Block, BlockByType, ImageRef } from "../../api/types.ts";
  import { RichTextEditor } from "../../editor/index.ts";
  import { ImageField, ImageListField } from "../../media/index.ts";
  import { Button, Field, SegmentedControl } from "../../portal/ui/index.ts";
  import { BLOCK_DESCRIPTIONS, BLOCK_LABELS } from "../blocks.ts";
  import { LIMITS } from "../limits.ts";
  import CtaFields from "./CtaFields.svelte";
  import LockedRow from "./LockedRow.svelte";
  import StatsFields from "./StatsFields.svelte";
  import TeamsFields from "./TeamsFields.svelte";
  import TextField from "./TextField.svelte";
  import WidthControl from "./WidthControl.svelte";
  import type { PageEditor } from "./state.svelte.ts";

  let {
    editor,
    block,
    compact = false,
    onremove,
    onpagesettings,
  }: {
    editor: PageEditor;
    block: Exclude<Block, { type: "cover" }>;
    compact?: boolean;
    onremove: (block: Block) => void;
    onpagesettings: () => void;
  } = $props();

  const problems = $derived(editor.problems.blocks[block.id] ?? {});
  const name = $derived(editor.committee.name);
  const disabled = $derived(!editor.writable);

  /** Saves new props for this block; `field` groups typing in one field into one undo step. */
  function set<T extends Block>(current: T, props: T["props"], field: string | null) {
    const next: Block = Object.assign({}, current, { props });
    editor.updateBlock(next, field === null ? null : `field:${current.id}:${field}`);
  }

  function galleryAltErrors(images: readonly ImageRef[]): (string | undefined)[] {
    return images.map((_, index) => problems[`images.${index}.alt`]);
  }

  const theme = $derived.by(() => {
    switch (block.type) {
      case "stats":
        return { label: "Rule color", value: `${name} accent`, swatch: editor.committee.color };
      case "quote":
        return { label: "Bar color", value: `${name} accent`, swatch: editor.committee.color };
      case "cta":
      case "work_sessions":
        return { label: "Background", value: `${name} tint`, swatch: editor.committee.tint };
      default:
        return { label: "Type and spacing", value: "ScottyLabs theme", swatch: undefined };
    }
  });
</script>

{#snippet gallery(current: Extract<Block, { type: "gallery" }>)}
  <div class="settings__field">
    <p class="settings__label" aria-hidden="true">Layout</p>
    <SegmentedControl
      label="Gallery layout"
      fullWidth
      options={[
        { value: "mosaic", label: "Mosaic" },
        { value: "thirds", label: "Thirds" },
      ]}
      value={current.props.layout}
      onchange={(layout: BlockByType["gallery"]["layout"]) => set(current, { ...current.props, layout }, null)}
    />
  </div>
  <ImageListField
    label="Photos"
    help="Three to six photos. The first is the large one in a mosaic. Drag to reorder, or use the arrow buttons."
    max={LIMITS.maxGallery}
    altRequired="all"
    coverLabel=""
    {disabled}
    value={current.props.images}
    error={problems.images}
    altErrors={galleryAltErrors(current.props.images)}
    onchange={(images) => set(current, { ...current.props, images }, null)}
  />
  <TextField
    label="Caption"
    optional
    value={current.props.caption}
    max={LIMITS.caption}
    error={problems.caption}
    onchange={(caption) => set(current, { ...current.props, caption }, "caption")}
  />
{/snippet}

<div class="settings">
  {#if !compact}
    <header class="settings__header">
      <h2 class="settings__title">{BLOCK_LABELS[block.type]}</h2>
      <p class="settings__description">{BLOCK_DESCRIPTIONS[block.type]}</p>
    </header>
  {/if}

  {#if !compact}
    <WidthControl {block} blocks={editor.doc.blocks} onchange={(width) => editor.setWidth(block.id, width)} />
  {/if}

  {#if block.type === "text"}
    <TextField
      label="Heading"
      value={block.props.heading}
      max={LIMITS.heading}
      error={problems.heading}
      onchange={(heading) => set(block, { ...block.props, heading }, "heading")}
    />
    {#if compact}
      <!-- Phones have no canvas to edit on, so the text gets its own field. -->
      <Field label="Body" error={problems.body}>
        <RichTextEditor
          value={block.props.body}
          maxChars={LIMITS.body}
          {disabled}
          onchange={(body) => set(block, { ...block.props, body }, "body")}
        />
      </Field>
    {:else}
      <div class="settings__field">
        <p class="settings__label">Body</p>
        <p class="settings__note">Click the text on the page to edit it in place.</p>
        {#if problems.body}<p class="settings__problem" role="alert">{problems.body}</p>{/if}
      </div>
    {/if}
    <hr class="settings__rule" />
    <div class="settings__field">
      <p class="settings__label">Formatting</p>
      <LockedRow label="Allowed" value="Bold, italic, links and lists" />
      <p class="settings__note">Fonts, sizes and colors are set by the theme.</p>
    </div>
  {:else if block.type === "image"}
    <ImageField
      label="Photo"
      value={block.props.image}
      {disabled}
      error={problems.image}
      altError={problems["image.alt"]}
      onchange={(image) => set(block, { ...block.props, image }, null)}
    />
    <TextField
      label="Caption"
      optional
      value={block.props.caption}
      max={LIMITS.caption}
      error={problems.caption}
      onchange={(caption) => set(block, { ...block.props, caption }, "caption")}
    />
  {:else if block.type === "gallery"}
    {@render gallery(block)}
  {:else if block.type === "video"}
    <TextField
      label="Video link"
      type="url"
      value={block.props.url}
      placeholder="https://www.youtube.com/watch?v="
      help="A YouTube or Vimeo link. The video loads only when a visitor chooses Play."
      error={problems.url}
      onchange={(url) => set(block, { ...block.props, url: url.trim() }, "url")}
    />
    <TextField
      label="Title"
      value={block.props.title}
      max={LIMITS.videoTitle}
      error={problems.title}
      onchange={(title) => set(block, { ...block.props, title }, "title")}
    />
    <ImageField
      label="Poster"
      help="Shown until someone plays the video."
      ratio={16 / 9}
      value={block.props.poster}
      {disabled}
      error={problems.poster}
      altError={problems["poster.alt"]}
      onchange={(poster) => set(block, { ...block.props, poster }, null)}
    />
  {:else if block.type === "stats"}
    <StatsFields
      items={block.props.items}
      {problems}
      onchange={(items, key) => set(block, { items }, key)}
    />
  {:else if block.type === "teams"}
    <TextField
      label="Heading"
      value={block.props.heading}
      max={LIMITS.heading}
      error={problems.heading}
      onchange={(heading) => set(block, { ...block.props, heading }, "heading")}
    />
    <TextField
      label="Introduction"
      optional
      multiline
      value={block.props.intro}
      max={LIMITS.shortText}
      error={problems.intro}
      onchange={(intro) => set(block, { ...block.props, intro }, "intro")}
    />
    <TeamsFields
      teams={block.props.teams}
      {problems}
      onchange={(teams, key) => set(block, { ...block.props, teams }, key)}
    />
  {:else if block.type === "projects"}
    <TextField
      label="Heading"
      value={block.props.heading}
      max={LIMITS.heading}
      error={problems.heading}
      onchange={(heading) => set(block, { heading }, "heading")}
    />
    <p class="settings__note">
      Shows {name}&rsquo;s published projects, featured first. <a href={`/portal/projects/?committee=${editor.slug}`}>Manage projects</a>
    </p>
  {:else if block.type === "quote"}
    <TextField
      label="Quote"
      multiline
      value={block.props.text}
      max={LIMITS.shortText}
      help="Leave out the quotation marks; the page adds them."
      error={problems.text}
      onchange={(text) => set(block, { ...block.props, text }, "text")}
    />
    <TextField
      label="Who said it"
      value={block.props.attribution}
      max={LIMITS.attribution}
      placeholder={`A ${name} member`}
      error={problems.attribution}
      onchange={(attribution) => set(block, { ...block.props, attribution }, "attribution")}
    />
  {:else if block.type === "work_sessions"}
    <p class="settings__note">Shows the meeting time, room and map link from the page settings.</p>
    <Button variant="outline" size="sm" onclick={onpagesettings}>Edit page settings</Button>
  {:else if block.type === "cta"}
    <TextField
      label="Heading"
      value={block.props.heading}
      max={LIMITS.heading}
      error={problems.heading}
      onchange={(heading) => set(block, { ...block.props, heading }, "heading")}
    />
    <TextField
      label="Text"
      multiline
      value={block.props.text}
      max={LIMITS.shortText}
      error={problems.text}
      onchange={(text) => set(block, { ...block.props, text }, "text")}
    />
    <CtaFields
      primary={block.props.primary}
      secondary={block.props.secondary}
      {problems}
      onchange={(buttons, key) => set(block, { ...block.props, ...buttons }, key)}
    />
  {/if}

  {#if problems[""]}<p class="settings__problem" role="alert">{problems[""]}</p>{/if}

  <hr class="settings__rule" />
  <div class="settings__field">
    <p class="settings__label">Theme</p>
    <LockedRow label={theme.label} value={theme.value} swatch={theme.swatch} />
  </div>

  {#if editor.writable && !compact}
    <hr class="settings__rule" />
    <div>
      <button type="button" class="settings__remove" onclick={() => onremove(block)}>Remove block</button>
    </div>
  {/if}
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .settings__header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .settings__title {
    margin: 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .settings__description,
  .settings__note {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;

    & a {
      color: var(--p-info-text);
      text-decoration: underline;
    }
  }
  .settings__field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .settings__label {
    margin: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .settings__problem {
    margin: 0;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .settings__rule {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--p-border);
  }
  .settings__remove {
    color: var(--p-danger-text);
    font-size: 0.9375rem;
    line-height: 22px;
    cursor: pointer;

    &:hover {
      text-decoration: underline;
    }
    &:focus-visible {
      border-radius: 2px;
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
</style>
