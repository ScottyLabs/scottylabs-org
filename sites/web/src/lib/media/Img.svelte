<script lang="ts">
  // A responsive image for an ImageRef. Uploads get a srcset of their stored widths; bundled
  // /seed images are used as they are. Width and height reserve the space so nothing shifts.
  //   <Img image={project.cover} sizes="(min-width: 768px) 236px, 96px" fit="cover" />
  import type { HTMLImgAttributes } from "svelte/elements";
  import type { ImageRef } from "../api/types.ts";
  import { src, srcset } from "./images.ts";

  type Props = Omit<HTMLImgAttributes, "src" | "srcset" | "alt" | "width" | "height"> & {
    image: ImageRef;
    /** Overrides the image's alt text. Pass "" when the image only decorates nearby text. */
    alt?: string;
    /** Which width the browser should pick, as in the `sizes` attribute. */
    sizes?: string;
    /** The width to request when the browser cannot use srcset. */
    fallbackWidth?: number;
    /** Overrides the intrinsic size used to reserve space. */
    width?: number;
    height?: number;
    /** Fills the box and crops, or fits inside it. Leave out for the natural size. */
    fit?: "cover" | "contain";
    /** Lazy by default. Use "eager" for the largest image above the fold. */
    loading?: "lazy" | "eager";
  };

  let {
    image,
    alt,
    sizes = "100vw",
    fallbackWidth = 960,
    width,
    height,
    fit,
    loading = "lazy",
    class: className,
    ...rest
  }: Props = $props();

  const candidates = $derived(srcset(image));
  const intrinsicWidth = $derived(width ?? image.width ?? undefined);
  const intrinsicHeight = $derived(height ?? image.height ?? undefined);
</script>

<img
  {...rest}
  class={["media-img", fit && `media-img--${fit}`, className]}
  src={src(image, fallbackWidth)}
  srcset={candidates}
  sizes={candidates === undefined ? undefined : sizes}
  alt={alt ?? image.alt}
  width={intrinsicWidth}
  height={intrinsicHeight}
  {loading}
  decoding="async"
/>

<style>
  .media-img {
    display: block;
    max-width: 100%;
    height: auto;
  }
  .media-img--cover,
  .media-img--contain {
    width: 100%;
    height: 100%;
  }
  .media-img--cover {
    object-fit: cover;
  }
  .media-img--contain {
    object-fit: contain;
  }
</style>
