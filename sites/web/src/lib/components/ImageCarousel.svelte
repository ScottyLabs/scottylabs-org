<script lang="ts">
  import type { Snippet } from "svelte";

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
    let prevRenderTime = performance.now();
    let totalElapsed = 0;
    let running = true;

    const updatePosition = (curRenderTime: number) => {
      if (!running) return;
      // cap the step so returning to a backgrounded tab doesn't trigger a burst of image loads
      totalElapsed += Math.min(500, curRenderTime - prevRenderTime);
      prevRenderTime = curRenderTime;
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

<div class={["carousel-container", direction === "right" && "carousel-container--flipped"]}>
  <div
    class="carousel"
    bind:this={carouselEl}
    style:height="{heightPx}px"
    style:gap={gapPx === undefined ? undefined : `${gapPx}px`}
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
</div>

<style>
  .carousel-container {
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
</style>
