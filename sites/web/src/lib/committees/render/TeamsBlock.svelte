<script lang="ts">
  // The committee's teams (spec C-9), each with its Slack channel. Phones show the first four
  // with a button for the rest.
  import type { BlockByType } from "../../api/types.ts";
  import slackMark from "../../assets/icons/socials/slack.svg";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";
  import { headingTag } from "./types.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["teams"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["teams"]) => void;
  } = $props();

  const uid = $props.id();
  const PHONE_LIMIT = 4;
  let expanded = $state(false);
  const hidden = $derived(Math.max(0, props.teams.length - PHONE_LIMIT));

  function setTeam(index: number, change: Partial<{ name: string; about: string }>) {
    const current = props.teams[index];
    if (current === undefined) return;
    onchange({ ...props, teams: props.teams.with(index, { ...current, ...change }) });
  }
</script>

<section class={["teams", expanded && "teams--expanded"]}>
  <div class="teams__heading">
    <EditableText
      as={headingTag(2, editor)}
      class="cb-heading"
      value={props.heading}
      {editor}
      {editable}
      label="Teams heading"
      placeholder="Add a heading"
      maxLength={LIMITS.heading}
      onchange={(heading) => onchange({ ...props, heading })}
    />
    <EditableText
      class="teams__intro"
      value={props.intro}
      {editor}
      {editable}
      label="Teams introduction"
      placeholder="Add a sentence about the teams (optional)"
      maxLength={LIMITS.shortText}
      onchange={(intro) => onchange({ ...props, intro })}
    />
  </div>
  {#if props.teams.length > 0}
    <ul class="teams__grid" id={`teams-${uid}`}>
      {#each props.teams as team, index (index)}
        <li class={["teams__card", index >= PHONE_LIMIT && "teams__card--more"]}>
          <EditableText
            as={headingTag(3, editor)}
            class="teams__name"
            value={team.name}
            {editor}
            {editable}
            label={`Name of team ${index + 1}`}
            placeholder="Team name"
            maxLength={LIMITS.teamName}
            onchange={(name) => setTeam(index, { name })}
          />
          <EditableText
            class="teams__about"
            value={team.about}
            {editor}
            {editable}
            label={`About team ${index + 1}`}
            placeholder="What the team does"
            maxLength={LIMITS.teamAbout}
            onchange={(about) => setTeam(index, { about })}
          />
          {#if team.channel !== ""}
            <p class="teams__channel">
              <img src={slackMark} alt="" width="12" height="12" />
              <span><span class="cb-sr-only">Slack channel </span>#{team.channel}</span>
            </p>
          {/if}
        </li>
      {/each}
    </ul>
    {#if hidden > 0}
      <button
        type="button"
        class="cb-btn cb-btn--outline cb-btn--sm teams__more"
        aria-expanded={expanded}
        aria-controls={`teams-${uid}`}
        tabindex={editor ? -1 : undefined}
        onclick={() => (expanded = !expanded)}
      >
        {expanded ? "Show fewer teams" : `Show all ${props.teams.length} teams`}
      </button>
    {/if}
  {:else if editor}
    <p class="teams__empty cb-placeholder">Add teams in the settings.</p>
  {/if}
</section>

<style>
  .teams {
    display: flex;
    flex-direction: column;
    gap: 24px;
    container: teams / inline-size;
  }
  .teams__heading {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .teams__heading :global(.teams__intro) {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .teams__grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 24px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .teams__card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    box-sizing: border-box;
    padding: 20px;
    border: 1px solid var(--cb-border);
    border-radius: 12px;
    background: #ffffff;
  }
  .teams__card :global(.teams__name) {
    margin: 0;
    color: var(--cb-text);
    font-size: 1rem;
    font-weight: 500;
    line-height: 24px;
  }
  .teams__card :global(.teams__about) {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .teams__channel {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    padding-top: 8px;
    color: var(--cb-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
    overflow-wrap: anywhere;
  }
  .teams__channel img {
    flex: none;
  }
  .teams__more {
    display: none;
    align-self: flex-start;
  }
  .teams__empty {
    margin: 0;
    font-size: 0.875rem;
  }

  @container teams (width < 900px) {
    .teams__grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @container teams (width < 480px) {
    .teams__grid {
      grid-template-columns: minmax(0, 1fr);
      gap: 12px;
    }
  }
  @container committee-page (width < 640px) {
    .teams__more {
      display: inline-flex;
    }
    .teams:not(.teams--expanded) .teams__card--more {
      display: none;
    }
  }
</style>
