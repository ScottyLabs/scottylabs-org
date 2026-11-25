<script lang="ts">
  // Three to six photos: a mosaic with one large photo and two beside it, or equal thirds. Photos
  // past the third share the row under them.
  import type { BlockByType } from "../../api/types.ts";
  import Images from "@lucide/svelte/icons/images";
  import Img from "../../media/Img.svelte";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";

  let {
    props,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["gallery"];
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["gallery"]) => void;
  } = $props();

  const extra = $derived(Math.max(0, props.images.length - 3));
</script>

{#if props.images.length > 0 || editor}
  <figure class="gallery">
    {#if props.images.length === 0}
      <div class="gallery__empty">
        <div class="cb-empty-media">
          <Images size={24} strokeWidth={1.75} aria-hidden="true" />
          <span>Add three to six photos in the settings.</span>
        </div>
      </div>
    {:else}
      <ul class={["gallery__grid", `gallery__grid--${props.layout}`, `gallery__grid--extra-${extra}`]}>
        {#each props.images as image, index (index)}
          <li class={["gallery__item", index === 0 && "gallery__item--lead", index >= 3 && "gallery__item--extra"]}>
            <Img
              {image}
              class="cb-photo"
              sizes={props.layout === "mosaic" && index === 0 ? "(min-width: 1280px) 792px, 66vw" : "(min-width: 1280px) 384px, 50vw"}
              fit="cover"
            />
          </li>
        {/each}
      </ul>
    {/if}
    <EditableText
      as="figcaption"
      class="cb-caption"
      value={props.caption}
      editor={editor && editable}
      {editable}
      label="Gallery caption"
      placeholder="Add a caption (optional)"
      maxLength={LIMITS.caption}
      onchange={(caption) => onchange({ ...props, caption })}
    />
  </figure>
{/if}

<style>
  .gallery {
    margin: 0;
  }
  .gallery__empty {
    aspect-ratio: 1200 / 400;
  }
  .gallery__grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 24px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  /* Hiding overflow lets the aspect ratio win over the photo's own height. */
  .gallery__item {
    display: block;
    grid-column: span 4;
    overflow: hidden;
    aspect-ratio: 4 / 3;
  }
  /* The mosaic: one photo across eight columns and two rows, two stacked beside it. */
  .gallery__grid--mosaic .gallery__item {
    aspect-ratio: 384 / 248;
  }
  .gallery__grid--mosaic .gallery__item--lead {
    position: relative;
    grid-column: span 8;
    grid-row: span 2;
    aspect-ratio: auto;
  }
  /* The large photo takes the height of the two beside it instead of setting it. */
  .gallery__grid--mosaic .gallery__item--lead :global(img) {
    position: absolute;
    inset: 0;
  }
  .gallery__grid--extra-1 .gallery__item--extra {
    grid-column: span 12;
    aspect-ratio: 16 / 7;
  }
  .gallery__grid--extra-2 .gallery__item--extra {
    grid-column: span 6;
  }

  @container committee-page (width < 640px) {
    .gallery__grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 12px;
    }
    .gallery__item,
    .gallery__grid--extra-2 .gallery__item--extra,
    .gallery__grid--extra-1 .gallery__item--extra {
      grid-column: span 1;
      aspect-ratio: 4 / 3;
    }
    .gallery__item--lead,
    .gallery__grid--mosaic .gallery__item--lead {
      grid-column: span 2;
      grid-row: auto;
      aspect-ratio: 4 / 3;
    }
    .gallery__grid--mosaic .gallery__item--lead :global(img) {
      position: static;
    }
    .gallery__empty {
      aspect-ratio: 4 / 3;
    }
  }
</style>
