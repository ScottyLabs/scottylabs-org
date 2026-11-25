<script lang="ts">
  // A strip of photos that drifts sideways. It stops while hovered and with its pause button,
  // which the visit remembers for every strip, and stands still as a row to scroll with reduced
  // motion (WCAG 2.2.2), like the ticker above it.
  import { onMount, type Snippet } from "svelte";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";

  const PAUSE_KEY = "sl-photos-paused";

  function readPaused(): boolean {
    try {
      return sessionStorage.getItem(PAUSE_KEY) === "1";
    } catch {
      return false;
    }
  }

  function writePaused(value: boolean): void {
    try {
      if (value) sessionStorage.setItem(PAUSE_KEY, "1");
      else sessionStorage.removeItem(PAUSE_KEY);
    } catch {
      // Blocked storage only means the choice is not remembered.
    }
  }

  function shuffleArray(ar: string[]) {
    const shuffledAr = [...ar];
    const N = shuffledAr.length;
    for (let i = 0; i < N; i++) {
      // rand([i,N-1])
      const randI = Math.floor(Math.random() * (N - i)) + i;
      [shuffledAr[i], shuffledAr[randI]] = [shuffledAr[randI], shuffledAr[i]];
    }
    return shuffledAr;
  }

  let {
    imageLinks,
    speedPxPerSecond,
    heightPx,
    gapPx,
    periodicTileInsert,
  }: {
    imageLinks: string[];
    speedPxPerSecond: number;
    heightPx: number;
    gapPx?: number;
    periodicTileInsert?: Snippet;
  } = $props();

  let reducedMotion = $state(false);
  let paused = $state(false);
  let hovering = $state(false);

  onMount(() => {
    paused = readPaused();
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = query.matches;
    const update = () => (reducedMotion = query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  });

  function togglePaused() {
    paused = !paused;
    writePaused(paused);
  }

  let carouselEl: HTMLDivElement | undefined = $state();
  let endOfCarouselEl: HTMLDivElement | undefined = $state();
  const direction = $derived(speedPxPerSecond > 0 ? "right" : "left");
  const LOADING_BUFFER_PX = 500;
  let imagesVisibleOnScreen = $state(5);
  const INITIAL_IMAGES_TO_LOAD = 10;

  $effect(() => {
    const updateCustomTilePositioning = () => {
      imagesVisibleOnScreen = Math.max(2, Math.ceil(window.innerWidth / 300));
    };
    window.addEventListener("resize", updateCustomTilePositioning);
    updateCustomTilePositioning();
    return () => {
      window.removeEventListener("resize", updateCustomTilePositioning);
    };
  });
  let tiles: string[] = $state([]);

  $effect(() => {
    const shuffledImageLinks = shuffleArray(imageLinks);
    const speed = speedPxPerSecond;
    const dir = direction;
    // tiles come from a client-side shuffle, so they can't be in the prerendered HTML
    tiles = Array.from(
      { length: INITIAL_IMAGES_TO_LOAD },
      (_, i) => shuffledImageLinks[i % shuffledImageLinks.length],
    );
    // With reduced motion the photos stand still in a row people can scroll themselves.
    if (reducedMotion) {
      if (carouselEl) carouselEl.style.translate = "";
      return;
    }
    let prevRenderTime = performance.now();
    let totalElapsed = 0;
    let running = true;

    const updatePosition = (curRenderTime: number) => {
      if (!running) return;
      // cap the step so returning to a backgrounded tab doesn't trigger a burst of image loads
      const step = Math.min(500, curRenderTime - prevRenderTime);
      prevRenderTime = curRenderTime;
      if (paused || hovering) {
        requestAnimationFrame(updatePosition);
        return;
      }
      totalElapsed += step;
      const offsetX = (totalElapsed / 1000) * speed;
      if (carouselEl && endOfCarouselEl) {
        const { left: posX } = endOfCarouselEl.getBoundingClientRect();

        if (
          (dir === "left" && posX <= window.innerWidth + LOADING_BUFFER_PX) ||
          (dir === "right" && posX >= -LOADING_BUFFER_PX && totalElapsed >= 1000)
        ) {
          // innerWidth accounts for page zoom
          // add in an additional image (or manual tile)

          // breaks if tiles leaving the viewport are ever removed
          tiles.push(shuffledImageLinks[tiles.length % shuffledImageLinks.length]);
        }
        carouselEl.style.translate = `${offsetX}px`;
      }
      requestAnimationFrame(updatePosition);
    };
    requestAnimationFrame(updatePosition);
    return () => {
      running = false;
    };
  });

  const renderedTiles = $derived(
    tiles.flatMap((src, i) => {
      if ((i + 2) % (imagesVisibleOnScreen - 1) === 0 && periodicTileInsert) {
        // insert tile
        return [
          { key: `image-${i}`, src },
          { key: `insert-${Math.floor(i / (imagesVisibleOnScreen - 1))}`, src: undefined },
        ];
      }
      return [{ key: `image-${i}`, src }];
    }),
  );
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class={[
    "carousel-container",
    direction === "right" && !reducedMotion && "carousel-container--flipped",
    reducedMotion && "carousel-container--still",
  ]}
  onpointerenter={() => (hovering = true)}
  onpointerleave={() => (hovering = false)}
>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="carousel"
    bind:this={carouselEl}
    style:height="{heightPx}px"
    style:gap={gapPx === undefined ? undefined : `${gapPx}px`}
    role={reducedMotion ? "region" : undefined}
    aria-label={reducedMotion ? "Photos" : undefined}
    tabindex={reducedMotion ? 0 : undefined}
  >
    {#each renderedTiles as { key, src } (key)}
      {#if src === undefined}
        <div style="flex-shrink: 0">
          {@render periodicTileInsert?.()}
        </div>
      {:else}
        <img class="carousel__image" {src} alt="" />
      {/if}
    {/each}
    <div bind:this={endOfCarouselEl}></div>
  </div>
  {#if !reducedMotion}
    <button
      type="button"
      class="carousel__pause"
      aria-label={paused ? "Play photos" : "Pause photos"}
      title={paused ? "Play" : "Pause"}
      onclick={togglePaused}
    >
      {#if paused}<Play size={14} strokeWidth={2} aria-hidden="true" />{:else}<Pause size={14} strokeWidth={2} aria-hidden="true" />{/if}
    </button>
  {/if}
</div>

<style>
  .carousel-container {
    position: relative;
    overflow: hidden;
    &.carousel-container--flipped {
      & > .carousel {
        margin-left: auto;
        flex-direction: row-reverse;
      }
    }
  }
  .carousel {
    display: flex;
    gap: 25px;
    align-items: stretch;
    /* this allows the div to function essentially as a pointer, and flex-direction dictates which side of the pointer the carousel should grow in */
    width: 0;
    translate: 0;

    & > .carousel__image {
      border-radius: var(--border-radius-base);
    }
  }
  .carousel-container--still {
    overflow: visible;

    & > .carousel {
      overflow-x: auto;
      width: auto;
      translate: none;
      overscroll-behavior-x: contain;

      &:focus-visible {
        outline: 2px solid var(--p-focus);
        outline-offset: 2px;
      }
    }
  }
  .carousel__pause {
    position: absolute;
    right: 12px;
    bottom: 12px;
    display: grid;
    place-items: center;
    box-sizing: border-box;
    width: 32px;
    height: 32px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-sm);
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
</style>
