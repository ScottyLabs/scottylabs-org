<script lang="ts">
  // The top of a committee page (spec C-7): the inspector label, name, tagline, the director, the
  // Join and Slack buttons and the cover picture.
  import type { Snippet } from "svelte";
  import type { BlockByType, CommitteeSettings } from "../../api/types.ts";
  import slackMark from "../../assets/icons/socials/slack.svg";
  import Img from "../../media/Img.svelte";
  import EditableText from "../EditableText.svelte";
  import { directorFor } from "../director.ts";
  import { LIMITS } from "../limits.ts";
  import { JOIN_URL, SLACK_URL } from "../links.ts";
  import type { CommitteeInfo } from "./types.ts";

  let {
    props,
    committee,
    settings,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["cover"];
    committee: CommitteeInfo;
    settings: CommitteeSettings;
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["cover"]) => void;
  } = $props();

  const director = $derived(directorFor(committee.slug));
  const channel = $derived(settings.slackChannel);
</script>

{#snippet action(href: string, variant: "filled" | "outline", children: Snippet)}
  {#if editor}
    <span class={["cb-btn", `cb-btn--${variant}`]}>{@render children()}</span>
  {:else}
    <a class={["cb-btn", `cb-btn--${variant}`]} {href} target="_blank" rel="noopener noreferrer">
      {@render children()}<span class="cb-sr-only"> (opens in a new tab)</span>
    </a>
  {/if}
{/snippet}

{#snippet joinLabel()}Join {committee.name}{/snippet}
{#snippet slackLabel()}
  <img class="cb-btn__icon" src={slackMark} alt="" width="16" height="16" />
  <span><span class="cb-sr-only">Slack channel </span>#{channel}</span>
{/snippet}

<header class="cover">
  <div class="cover__title-row">
    <div class="cover__title">
      <p class="cb-inspector">[committee="{committee.slug}"]</p>
      <h1 class="cover__name">{committee.name}</h1>
      <EditableText
        class="cover__tagline"
        value={props.tagline}
        {editor}
        {editable}
        label="Tagline"
        placeholder="Add a tagline that says what the committee does"
        maxLength={LIMITS.tagline}
        onchange={(tagline) => onchange({ ...props, tagline })}
      />
    </div>
    <div class="cover__contact">
      {#if director}
        <div class="cover__director">
          {#if director.photoUrl}
            <img class="cover__avatar" src={director.photoUrl} alt="" width="44" height="44" loading="lazy" />
          {:else}
            <span class="cover__avatar cover__avatar--initials" aria-hidden="true">{director.initials}</span>
          {/if}
          <p class="cover__person">
            <span class="cover__person-name">{director.name}</span>
            <span class="cover__person-role">{director.role}</span>
          </p>
        </div>
      {/if}
      <div class="cover__actions">
        {@render action(JOIN_URL, "filled", joinLabel)}
        {#if channel}
          {@render action(SLACK_URL, "outline", slackLabel)}
        {/if}
      </div>
    </div>
  </div>
  <div class="cover__picture">
    {#if props.image}
      <Img
        image={props.image}
        class="cb-photo cover__img"
        sizes="(min-width: 1280px) 1200px, 100vw"
        fallbackWidth={1920}
        fit="cover"
        loading="eager"
        fetchpriority="high"
      />
    {:else if editor}
      <div class="cb-empty-media">Add a cover picture in the page settings.</div>
    {/if}
  </div>
</header>

<style>
  .cover {
    display: flex;
    flex-direction: column;
    gap: 40px;
  }
  .cover__title-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px 40px;
  }
  .cover__title {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    max-width: 720px;
  }
  .cover__name {
    margin: 0;
    color: var(--cb-text);
    font-size: 5rem;
    font-weight: 600;
    line-height: 84px;
    letter-spacing: -0.025em;
  }
  .cover__title :global(.cover__tagline) {
    margin: 0;
    color: var(--cb-text-secondary);
    font-size: 1.375rem;
    font-weight: 400;
    line-height: 32px;
  }
  /* Stays at the right edge, also when a long tagline pushes it onto its own line. */
  .cover__contact {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 16px;
    margin-left: auto;
  }
  .cover__director {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .cover__avatar {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--cb-tint);
    object-fit: cover;
  }
  .cover__avatar--initials {
    display: grid;
    place-items: center;
    color: var(--cb-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .cover__person {
    display: flex;
    flex-direction: column;
    margin: 0;
  }
  .cover__person-name {
    color: var(--cb-text);
    font-size: 1rem;
    font-weight: 500;
    line-height: 24px;
  }
  .cover__person-role {
    color: var(--cb-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .cover__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  /* Hiding overflow lets the aspect ratio win over the photo's own height. */
  .cover__picture {
    overflow: hidden;
    aspect-ratio: 1200 / 520;
  }
  .cover__picture :global(.cover__img) {
    border-radius: 20px;
  }

  @container committee-page (width < 640px) {
    .cover {
      gap: 24px;
    }
    .cover__title-row {
      flex-direction: column;
      align-items: stretch;
    }
    .cover__name {
      font-size: 3rem;
      line-height: 52px;
    }
    .cover__title :global(.cover__tagline) {
      font-size: 1.125rem;
      line-height: 28px;
    }
    .cover__contact {
      align-items: flex-start;
      margin-left: 0;
    }
    .cover__picture {
      aspect-ratio: 280 / 175;
    }
    .cover__picture :global(.cover__img) {
      border-radius: 16px;
    }
  }
  @container committee-page (640px <= width < 960px) {
    .cover__name {
      font-size: 4rem;
      line-height: 68px;
    }
  }
</style>
