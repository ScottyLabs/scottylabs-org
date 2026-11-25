<script lang="ts">
  // A YouTube or Vimeo video that loads only when the visitor presses play, from the
  // privacy-enhanced player. Until then nothing is requested from the video site.
  //   <VideoPlayer url={project.videoUrl} title="TartanHacks video" poster={project.media[0]} />
  import { tick } from "svelte";
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import Play from "@lucide/svelte/icons/play";
  import type { ImageRef } from "../api/types.ts";
  import Img from "../media/Img.svelte";
  import { videoEmbed } from "../media/video.ts";

  let {
    url,
    title,
    poster = null,
  }: {
    url: string;
    /** Names the video for the play button and the player. */
    title: string;
    poster?: ImageRef | null;
  } = $props();

  const embed = $derived(videoEmbed(url));
  let playing = $state(false);
  let frame: HTMLIFrameElement | undefined = $state();

  async function play(): Promise<void> {
    playing = true;
    await tick();
    frame?.focus();
  }
</script>

{#if embed === null}
  <a class="video-link" href={url} target="_blank" rel="noopener noreferrer">
    Watch the video
    <ArrowUpRight size={16} strokeWidth={2} aria-hidden="true" />
    <span class="p-sr-only">(opens in a new tab)</span>
  </a>
{:else}
  <div class="video">
    {#if playing}
      <iframe
        bind:this={frame}
        class="video__frame"
        src={embed.embedUrl}
        {title}
        allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
        allowfullscreen
        referrerpolicy="strict-origin-when-cross-origin"
      ></iframe>
    {:else}
      <button type="button" class="video__start" onclick={play}>
        {#if poster !== null}
          <Img image={poster} alt="" sizes="(min-width: 1200px) 768px, 100vw" fit="cover" loading="lazy" />
        {/if}
        <span class="video__scrim" aria-hidden="true"></span>
        <span class="video__play" aria-hidden="true">
          <Play size={28} strokeWidth={2} />
        </span>
        <span class="video__label">
          Play {title}
          <span class="p-sr-only">from {embed.providerName}</span>
        </span>
      </button>
    {/if}
  </div>
  {#if !playing}
    <p class="video__note">The video loads from {embed.providerName} when you press play.</p>
  {/if}
{/if}

<style>
  .video {
    position: relative;
    overflow: hidden;
    aspect-ratio: 16 / 9;
    border-radius: 20px;
    background: #000;
  }
  .video__frame {
    display: block;
    width: 100%;
    height: 100%;
    border: 0;
  }
  .video__start {
    position: absolute;
    inset: 0;
    display: block;
    cursor: pointer;

    &:focus-visible {
      outline: 3px solid var(--p-focus);
      outline-offset: -3px;
      border-radius: 20px;
    }
  }
  .video__scrim {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    transition: background-color var(--p-duration) var(--p-ease);
  }
  .video__start:hover .video__scrim {
    background: rgba(0, 0, 0, 0.35);
  }
  .video__play {
    position: absolute;
    top: 50%;
    left: 50%;
    display: grid;
    place-items: center;
    width: 76px;
    height: 76px;
    border-radius: 50%;
    background: #fff;
    color: var(--p-text);
    transform: translate(-50%, -50%);
  }
  .video__label {
    position: absolute;
    left: 24px;
    bottom: 24px;
    max-width: calc(100% - 48px);
    overflow: hidden;
    padding: 6px 12px;
    border-radius: var(--p-radius-pill);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-family: var(--p-font);
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 20px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .video__note {
    margin: 10px 0 0;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .video-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--p-text);
    font-weight: 600;
    text-decoration: underline;
    text-underline-offset: 3px;

    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
      border-radius: 4px;
    }
  }
  @media (max-width: 600px) {
    .video__play {
      width: 56px;
      height: 56px;
    }
    .video__label {
      left: 14px;
      bottom: 14px;
    }
  }
</style>
