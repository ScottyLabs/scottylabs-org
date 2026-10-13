<script lang="ts">
  import emblaCarouselSvelte, { type EmblaCarouselSvelteType } from "embla-carousel-svelte";
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import type { CarouselPhoto } from "#lib/data/history.ts";

  type EmblaCarouselType = Parameters<
    NonNullable<NonNullable<EmblaCarouselSvelteType["$$_attributes"]>["onemblaInit"]>
  >[0]["detail"];

  const HISTORY_ASSETS_BASE_URL = "https://assets.scottylabs.org/team-page/history/";

  let { photos }: { photos: CarouselPhoto[] } = $props();

  let emblaApi = $state<EmblaCarouselType>();
  let canPrev = $state(false);
  let canNext = $state(false);

  $effect(() => {
    const api = emblaApi;
    if (api === undefined) return;
    const onChange = () => {
      canPrev = api.canScrollPrev();
      canNext = api.canScrollNext();
    };
    onChange();
    api.on("select", onChange).on("reInit", onChange);
    return () => {
      api.off("select", onChange).off("reInit", onChange);
    };
  });

  const scrollPrev = () => {
    emblaApi?.scrollPrev();
  };
  const scrollNext = () => {
    emblaApi?.scrollNext();
  };
</script>

<div class="root">
  <div
    class="viewport"
    use:emblaCarouselSvelte={{
      options: { align: "start", containScroll: "trimSnaps", dragFree: true },
      plugins: [],
    }}
    onemblaInit={(event) => (emblaApi = event.detail)}
  >
    <div class="container">
      {#each photos as photo (photo.filename)}
        <div class="slide">
          <img
            src={`${HISTORY_ASSETS_BASE_URL}${photo.filename}`}
            alt={photo.caption ?? ""}
            class="image"
            loading="lazy"
            decoding="async"
          />
          {#if photo.caption !== undefined}
            <div class="caption">{photo.caption}</div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
  <button
    type="button"
    class="button prev"
    onclick={scrollPrev}
    disabled={!canPrev}
    aria-label="Previous"
  >
    <ChevronLeft size={20} />
  </button>
  <button
    type="button"
    class="button next"
    onclick={scrollNext}
    disabled={!canNext}
    aria-label="Next"
  >
    <ChevronRight size={20} />
  </button>
</div>

<style>
  .root {
    position: relative;
  }

  .viewport {
    overflow: hidden;
    border-radius: 12px;
  }

  .container {
    display: flex;
    gap: 12px;
    align-items: stretch;
  }

  .slide {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  .image {
    height: 220px;
    @media (max-width: 768px) {
      height: 180px;
    }
    width: auto;
    max-width: 100%;
    object-fit: contain;
    display: block;
    border-radius: 8px;
    background: #fff;
  }

  .caption {
    margin-top: 8px;
    font-size: 12px;
    color: #6b7280;
    font-family: Satoshi, sans-serif;
    line-height: 1.3;
    max-width: 320px;
  }

  .button {
    position: absolute;
    top: 110px;
    @media (max-width: 768px) {
      top: 90px;
    }
    transform: translateY(-50%);
    width: 36px;
    height: 36px;
    border-radius: 9999px;
    border: 1px solid #b0b0b0;
    background: #fff;
    color: #07123b;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 2px 6px 0 rgba(0, 0, 0, 0.12);
    z-index: 2;
    padding: 0;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .button:hover:not(:disabled) {
    transform: translateY(-50%) scale(1.05);
    box-shadow: 0 4px 10px 0 rgba(0, 0, 0, 0.16);
  }

  .button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .prev {
    left: -12px;
  }

  .next {
    right: -12px;
  }
</style>
