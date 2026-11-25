<script lang="ts">
  // A project's photos: one large picture and a rail of thumbnails that swaps it (spec P-9). The
  // rail sits to the left on wide screens and below on narrow ones. Thumbnails are buttons; the
  // arrow keys move between them, and the change is announced.
  //   <ProjectGallery images={project.media} name={project.name} committee={project.committee} />
  import type { CommitteeSlug, ImageRef } from "../api/types.ts";
  import Img from "../media/Img.svelte";
  import { COMMITTEES } from "../portal/committees.ts";

  let {
    images,
    name,
    committee,
    loading = "eager",
  }: {
    images: readonly ImageRef[];
    name: string;
    committee: CommitteeSlug;
    loading?: "lazy" | "eager";
  } = $props();

  let selected = $state(0);
  const thumbs: HTMLButtonElement[] = $state([]);
  const index = $derived(Math.min(selected, Math.max(images.length - 1, 0)));
  const current = $derived(images[index]);
  const style = $derived(COMMITTEES[committee]);
  const announcement = $derived(
    current === undefined || images.length < 2
      ? ""
      : `Photo ${(index + 1).toString()} of ${images.length.toString()}${current.alt.trim() === "" ? "" : `: ${current.alt}`}`,
  );

  function show(next: number): void {
    selected = Math.max(0, Math.min(next, images.length - 1));
  }

  function onkeydown(event: KeyboardEvent, position: number): void {
    const last = images.length - 1;
    const moves: Record<string, number> = {
      ArrowRight: position === last ? 0 : position + 1,
      ArrowDown: position === last ? 0 : position + 1,
      ArrowLeft: position === 0 ? last : position - 1,
      ArrowUp: position === 0 ? last : position - 1,
      Home: 0,
      End: last,
    };
    const next = moves[event.key];
    if (next === undefined) return;
    event.preventDefault();
    show(next);
    thumbs[next]?.focus();
  }
</script>

<div class="gallery-host">
<div class={["gallery", images.length > 1 && "gallery--rail"]}>
  <figure class="gallery__main">
    {#if current !== undefined}
      <Img
        image={current}
        sizes="(min-width: 1200px) 680px, (min-width: 900px) 58vw, 100vw"
        fallbackWidth={960}
        fit="cover"
        {loading}
      />
      {#if images.length > 1}
        <span class="gallery__counter" aria-hidden="true">
          {index + 1} / {images.length}
        </span>
      {/if}
    {:else}
      <div class="gallery__empty" style:--tint={style.tint} style:--color={style.color}>
        <span aria-hidden="true">{name}</span>
      </div>
    {/if}
  </figure>

  {#if images.length > 1}
    <div class="gallery__rail" role="group" aria-label={`Photos of ${name}`}>
      {#each images as image, position (`${image.src}-${position.toString()}`)}
        <button
          bind:this={thumbs[position]}
          type="button"
          class={["gallery__thumb", position === index && "gallery__thumb--current"]}
          aria-label={`Show photo ${(position + 1).toString()} of ${images.length.toString()}`}
          aria-pressed={position === index ? "true" : "false"}
          onclick={() => show(position)}
          onkeydown={(event) => onkeydown(event, position)}
        >
          <Img image={image} alt="" sizes="72px" fallbackWidth={480} fit="cover" loading="lazy" />
        </button>
      {/each}
    </div>
    <p class="p-sr-only" aria-live="polite">{announcement}</p>
  {/if}
</div>
</div>

<style>
  .gallery-host {
    container-type: inline-size;
    min-width: 0;
  }
  .gallery {
    display: grid;
    gap: 12px;
    min-width: 0;
  }
  .gallery__main {
    position: relative;
    overflow: hidden;
    margin: 0;
    aspect-ratio: 4 / 3;
    border-radius: 20px;
    background: var(--p-surface-muted);

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px var(--p-border);
      pointer-events: none;
    }
  }
  .gallery__counter {
    position: absolute;
    right: 16px;
    bottom: 16px;
    padding: 5px 10px;
    border-radius: var(--p-radius-pill);
    background: var(--p-inverse);
    color: var(--p-on-inverse);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .gallery__empty {
    display: flex;
    align-items: flex-end;
    height: 100%;
    padding: 24px;
    box-sizing: border-box;
    background: var(--tint);
    color: var(--p-text);
    font-size: clamp(2rem, 6cqi, 3.5rem);
    font-weight: 600;
    line-height: 1.1;
    letter-spacing: -0.03em;
  }
  .gallery__rail {
    display: flex;
    gap: 10px;
    overflow-x: auto;
    padding: 2px;
    scrollbar-width: none;

    &::-webkit-scrollbar {
      display: none;
    }
  }
  .gallery__thumb {
    position: relative;
    display: block;
    flex: none;
    overflow: hidden;
    width: 72px;
    height: 72px;
    border-radius: 12px;
    cursor: pointer;
    background: var(--p-surface-muted);

    &::after {
      content: "";
      position: absolute;
      inset: 0;
      border-radius: inherit;
      box-shadow: inset 0 0 0 1px var(--p-border);
      transition: box-shadow var(--p-duration) var(--p-ease);
    }
    &:hover::after {
      box-shadow: inset 0 0 0 1px var(--p-text-secondary);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .gallery__thumb--current::after,
  .gallery__thumb--current:hover::after {
    box-shadow: inset 0 0 0 2px var(--p-text);
  }

  /* Wide: the rail runs down the left side of the picture (Figma 10:217). */
  @container (min-width: 560px) {
    .gallery--rail {
      grid-template-columns: 72px minmax(0, 1fr);
      gap: 16px;
      align-items: start;
    }
    .gallery--rail .gallery__main {
      grid-column: 2;
      grid-row: 1;
    }
    .gallery--rail .gallery__rail {
      grid-column: 1;
      grid-row: 1;
      flex-direction: column;
      gap: 12px;
      overflow: visible;
      padding: 0;
    }
  }
</style>
