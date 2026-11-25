<script lang="ts">
  // A YouTube or Vimeo video. Visitors see the poster first; the privacy-enhanced player loads
  // only after they choose Play, so nothing is sent to those sites before then.
  import type { BlockByType } from "../../api/types.ts";
  import Play from "@lucide/svelte/icons/play";
  import Video from "@lucide/svelte/icons/video";
  import Img from "../../media/Img.svelte";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";
  import { videoEmbed } from "../video.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["video"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["video"]) => void;
  } = $props();

  const embed = $derived(videoEmbed(props.url));
  let playing = $state(false);
  let player: HTMLIFrameElement | undefined = $state();

  // A new link stops the old video.
  $effect(() => {
    void props.url;
    playing = false;
  });

  $effect(() => {
    if (playing) player?.focus();
  });

  const name = $derived(props.title === "" ? "the video" : props.title);
</script>

{#if embed !== null || editor}
  <figure class="video">
    <div class="video__frame">
      {#if playing && embed !== null && !editor}
        <iframe
          bind:this={player}
          class="video__player"
          src={embed.src}
          title={props.title === "" ? `${embed.provider} video` : props.title}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowfullscreen
          referrerpolicy="strict-origin-when-cross-origin"
        ></iframe>
      {:else}
        {#if props.poster}
          <Img image={props.poster} class="cb-photo" alt="" sizes="(min-width: 1280px) 792px, 100vw" fit="cover" />
        {:else}
          <div class="video__blank"></div>
        {/if}
        <span class="video__scrim" aria-hidden="true"></span>
        {#if editor}
          <span class="video__play" aria-hidden="true"><Play size={26} strokeWidth={2} /></span>
          {#if embed === null}
            <span class="video__note">
              <Video size={16} strokeWidth={2} aria-hidden="true" />
              Add a YouTube or Vimeo link in the settings.
            </span>
          {/if}
        {:else}
          <button type="button" class="video__play" onclick={() => (playing = true)}>
            <Play size={26} strokeWidth={2} aria-hidden="true" />
            <span class="cb-sr-only">Play {name} from {embed?.provider}</span>
          </button>
        {/if}
      {/if}
    </div>
    <figcaption class="video__caption">
      <EditableText
        as="span"
        class="video__title"
        value={props.title}
        {editor}
        {editable}
        label="Video title"
        placeholder="Add a title"
        maxLength={LIMITS.videoTitle}
        onchange={(title) => onchange({ ...props, title })}
      />
      <span class="video__provider">{embed?.provider ?? "Video"} video</span>
    </figcaption>
  </figure>
{/if}

<style>
  .video {
    margin: 0;
  }
  .video__frame {
    position: relative;
    overflow: hidden;
    aspect-ratio: 16 / 9;
    border-radius: 16px;
  }
  .video__blank {
    width: 100%;
    height: 100%;
    background: #2b2b2b;
  }
  .video__scrim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.28);
    pointer-events: none;
  }
  .video__play {
    position: absolute;
    top: 50%;
    left: 50%;
    display: grid;
    place-items: center;
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: #ffffff;
    color: #1e1e1e;
    transform: translate(-50%, -50%);
    cursor: pointer;
    transition: transform var(--p-duration) var(--p-ease);
  }
  button.video__play:hover {
    transform: translate(-50%, -50%) scale(1.06);
  }
  button.video__play:focus-visible {
    outline: 3px solid #ffffff;
    outline-offset: 3px;
    box-shadow: 0 0 0 6px var(--p-focus);
  }
  .video__note {
    position: absolute;
    right: 12px;
    bottom: 12px;
    left: 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.7);
    color: #ffffff;
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .video__player {
    display: block;
    width: 100%;
    height: 100%;
    border: 0;
  }
  .video__caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 14px;
  }
  .video__caption :global(.video__title) {
    color: var(--cb-text);
    font-size: 1rem;
    font-weight: 500;
    line-height: 24px;
  }
  .video__provider {
    color: var(--cb-text-tertiary);
    font-size: 0.875rem;
    line-height: 20px;
  }

  @media (prefers-reduced-motion: reduce) {
    .video__play {
      transition: none;
    }
  }
</style>
