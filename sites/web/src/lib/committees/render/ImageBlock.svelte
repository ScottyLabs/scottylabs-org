<script lang="ts">
  // One photo with an optional caption. Narrow blocks crop taller, wide ones to 16:9.
  import type { BlockByType, BlockWidth } from "../../api/types.ts";
  import ImagePlus from "@lucide/svelte/icons/image-plus";
  import Img from "../../media/Img.svelte";
  import EditableText from "../EditableText.svelte";
  import { LIMITS } from "../limits.ts";

  let {
    props,
    width,
    editor,
    editable,
    onchange,
  }: {
    props: BlockByType["image"];
    width: BlockWidth;
    editor: boolean;
    editable: boolean;
    onchange: (props: BlockByType["image"]) => void;
  } = $props();

  const sizes = $derived(
    width === 12 ? "(min-width: 1280px) 1200px, 100vw" : `(min-width: 1280px) ${Math.round((1200 * width) / 12)}px, (min-width: 640px) ${Math.round((100 * width) / 12)}vw, 100vw`,
  );
</script>

{#if props.image || editor}
  <figure class={["image", width === 4 && "image--narrow"]}>
    <div class="image__frame">
      {#if props.image}
        <Img image={props.image} class="cb-photo" {sizes} fit="cover" />
      {:else}
        <div class="cb-empty-media">
          <ImagePlus size={24} strokeWidth={1.75} aria-hidden="true" />
          <span>Add a photo</span>
          <span class="image__hint">JPG, PNG or WebP, up to 5 MB</span>
        </div>
      {/if}
    </div>
    <EditableText
      as="figcaption"
      class="cb-caption"
      value={props.caption}
      editor={editor && editable}
      {editable}
      label="Caption"
      placeholder="Add a caption (optional)"
      maxLength={LIMITS.caption}
      onchange={(caption) => onchange({ ...props, caption })}
    />
  </figure>
{/if}

<style>
  .image {
    margin: 0;
  }
  /* Hiding overflow lets the aspect ratio win over the photo's own height. */
  .image__frame {
    overflow: hidden;
    aspect-ratio: 16 / 9;
  }
  .image--narrow .image__frame {
    aspect-ratio: 384 / 420;
  }
  .image__hint {
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }

  @container committee-page (width < 640px) {
    .image--narrow .image__frame {
      aspect-ratio: 4 / 3;
    }
  }
</style>
