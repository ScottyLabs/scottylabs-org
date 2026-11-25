<script lang="ts">
  // Settings for the whole page (Figma 109:7058): the tagline and cover picture, what the
  // Committees page says, when and where the committee meets, the Slack channel, the theme rows
  // that cannot change, and how many blocks the page uses.
  import type { Block, CommitteeSettings } from "../../api/types.ts";
  import { ImageField } from "../../media/index.ts";
  import { MAX_BLOCKS } from "../blocks.ts";
  import { LIMITS, cleanChannel } from "../limits.ts";
  import LockedRow from "./LockedRow.svelte";
  import TextField from "./TextField.svelte";
  import { altTextField } from "./photo-undo.ts";
  import type { PageEditor } from "./state.svelte.ts";

  let { editor, compact = false }: { editor: PageEditor; compact?: boolean } = $props();

  const settings = $derived(editor.doc.settings);
  const cover = $derived(editor.doc.blocks[0]);
  const coverProblems = $derived(cover === undefined ? {} : (editor.problems.blocks[cover.id] ?? {}));
  const settingProblems = $derived(editor.problems.settings);
  const disabled = $derived(!editor.writable);
  const used = $derived(editor.doc.blocks.length);

  function setCover(change: Partial<Extract<Block, { type: "cover" }>["props"]>, key: string | null) {
    if (cover?.type !== "cover") return;
    editor.updateBlock({ ...cover, props: { ...cover.props, ...change } }, key === null ? null : `cover:${key}`);
  }

  function setSetting(change: Partial<CommitteeSettings>, key: string) {
    editor.updateSettings(change, `settings:${key}`);
  }
</script>

<div class="page-settings">
  <header class="page-settings__header">
    <h2 class="page-settings__title">Page</h2>
    <p class="page-settings__description">
      Settings for the whole {editor.committee.name} page.{compact ? "" : " Select a block to change it."}
    </p>
  </header>

  {#if cover?.type === "cover"}
    <TextField
      label="Tagline"
      multiline
      value={cover.props.tagline}
      max={LIMITS.tagline}
      help="One sentence under the committee name."
      error={coverProblems.tagline}
      onchange={(tagline) => setCover({ tagline }, "tagline")}
    />
    <ImageField
      label="Cover picture"
      help="A wide photo, at least 1200 pixels across."
      ratio={1200 / 520}
      value={cover.props.image}
      {disabled}
      error={coverProblems.image}
      altError={coverProblems["image.alt"]}
      onchange={(image) => setCover({ image }, altTextField("image", cover.props.image, image))}
    />
  {/if}

  <hr class="page-settings__rule" />
  <section class="page-settings__group" aria-labelledby="index-settings">
    <h3 id="index-settings" class="page-settings__heading">On the Committees page</h3>
    <TextField
      label="Description"
      multiline
      value={settings.indexDescription}
      max={LIMITS.indexDescription}
      error={settingProblems.indexDescription}
      onchange={(indexDescription) => setSetting({ indexDescription }, "indexDescription")}
    />
    <TextField
      label="Teams line"
      value={settings.teamsLine}
      max={LIMITS.teamsLine}
      placeholder="3 teams: Logistics, TartanHacks and NOVA"
      error={settingProblems.teamsLine}
      onchange={(teamsLine) => setSetting({ teamsLine }, "teamsLine")}
    />
  </section>

  <hr class="page-settings__rule" />
  <section class="page-settings__group" aria-labelledby="session-settings">
    <h3 id="session-settings" class="page-settings__heading">Work sessions</h3>
    <TextField
      label="Meeting time"
      value={settings.meetingTime}
      max={LIMITS.meetingTime}
      placeholder="Saturdays, 4&ndash;6 PM"
      error={settingProblems.meetingTime}
      onchange={(meetingTime) => setSetting({ meetingTime }, "meetingTime")}
    />
    <TextField
      label="Room"
      value={settings.meetingRoom}
      max={LIMITS.meetingRoom}
      placeholder="Tepper 2700"
      error={settingProblems.meetingRoom}
      onchange={(meetingRoom) => setSetting({ meetingRoom }, "meetingRoom")}
    />
    <TextField
      label="Map link"
      type="url"
      optional
      value={settings.mapUrl ?? ""}
      placeholder="https://maps.scottylabs.org/TEP-2700"
      help="The room in CMU Maps."
      error={settingProblems.mapUrl}
      onchange={(mapUrl) => setSetting({ mapUrl: mapUrl.trim() === "" ? null : mapUrl.trim() }, "mapUrl")}
    />
    <TextField
      label="Slack channel"
      optional
      prefix="#"
      value={settings.slackChannel ?? ""}
      max={LIMITS.channel}
      placeholder={editor.slug}
      help="Shown beside the Join button."
      error={settingProblems.slackChannel}
      onchange={(channel) => {
        const slackChannel = cleanChannel(channel);
        setSetting({ slackChannel: slackChannel === "" ? null : slackChannel }, "slackChannel");
      }}
    />
  </section>

  <hr class="page-settings__rule" />
  <section class="page-settings__group" aria-labelledby="theme-settings">
    <h3 id="theme-settings" class="page-settings__heading">Theme</h3>
    <LockedRow label="Accent color" value={editor.committee.name} swatch={editor.committee.color} />
    <LockedRow label="Type and spacing" value="ScottyLabs theme" />
    <p class="page-settings__note">
      Colors, type and spacing come from the ScottyLabs theme, so every committee page matches the site. You
      choose the blocks, their order and their width.
    </p>
  </section>

  <hr class="page-settings__rule" />
  <section class="page-settings__group" aria-labelledby="blocks-used">
    <h3 id="blocks-used" class="page-settings__heading">Blocks on this page</h3>
    <div
      class="page-settings__meter"
      role="meter"
      aria-labelledby="blocks-used"
      aria-valuemin={0}
      aria-valuemax={MAX_BLOCKS}
      aria-valuenow={used}
      aria-valuetext={`${used} of ${MAX_BLOCKS} blocks used`}
    >
      <span style:width={`${(100 * Math.min(used, MAX_BLOCKS)) / MAX_BLOCKS}%`}></span>
    </div>
    <p class="page-settings__note">{used} of {MAX_BLOCKS} blocks used, including the cover.</p>
  </section>
</div>

<style>
  .page-settings {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .page-settings__header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .page-settings__title {
    margin: 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .page-settings__description,
  .page-settings__note {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .page-settings__group {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .page-settings__heading {
    margin: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .page-settings__rule {
    width: 100%;
    margin: 0;
    border: 0;
    border-top: 1px solid var(--p-border);
  }
  .page-settings__meter {
    overflow: hidden;
    height: 6px;
    border-radius: 3px;
    background: var(--p-surface-strong);

    & span {
      display: block;
      height: 100%;
      border-radius: 3px;
      background: var(--p-inverse);
    }
  }
</style>
