<script lang="ts">
  // The ScottyLabs selection box: a 2 px blue outline with four square handles and a mono label
  // above the top left corner, like an object selected in a design tool.
  //
  // Put it last inside an element with `class="sl-select-host"` and a position. It shows while the
  // host is hovered, or while an element inside it marked `data-selection-target` (usually the
  // link that covers the host) has keyboard focus, when it turns the darker focus blue:
  //   <article class="sl-select-host" style="position: relative">
  //     <h3><a href="/projects/cmu-maps/" data-selection-target>CMU Maps</a></h3>
  //     <SelectionBox label="/projects/cmu-maps" />
  //   </article>
  // Pass `active` to show it from code instead, such as for a paused ticker item.

  let {
    label,
    active = false,
    inset = 0,
    class: className,
  }: {
    /** Such as "/projects/cmu-maps" or a link's host name. Decorative: hidden from assistive technology. */
    label?: string;
    active?: boolean;
    /** Distance from the host's edges in pixels. Negative values draw the box outside the host. */
    inset?: number;
    class?: string;
  } = $props();
</script>

<span
  class={["sl-selection", active && "sl-selection--active", className]}
  style:--sl-inset={`${inset.toString()}px`}
  aria-hidden="true"
>
  <span class="sl-selection__handle sl-selection__handle--tl"></span>
  <span class="sl-selection__handle sl-selection__handle--tr"></span>
  <span class="sl-selection__handle sl-selection__handle--bl"></span>
  <span class="sl-selection__handle sl-selection__handle--br"></span>
  {#if label !== undefined && label !== ""}
    <span class="sl-selection__label">{label}</span>
  {/if}
</span>

<style>
  .sl-selection {
    --sl-color: var(--p-selection);
    position: absolute;
    inset: var(--sl-inset);
    z-index: 3;
    box-sizing: border-box;
    border: 2px solid var(--sl-color);
    opacity: 0;
    pointer-events: none;
    transition: opacity 120ms var(--p-ease);
  }
  @media (hover: hover) {
    :global(.sl-select-host:hover) > .sl-selection {
      opacity: 1;
    }
  }
  /* #24A4FF is under 3:1 on white, so keyboard focus uses the focus blue, which is not. */
  :global(.sl-select-host:has([data-selection-target]:focus-visible)) > .sl-selection {
    --sl-color: var(--p-focus);
    opacity: 1;
  }
  .sl-selection--active {
    opacity: 1;
  }

  .sl-selection__handle {
    position: absolute;
    box-sizing: border-box;
    width: 8px;
    height: 8px;
    border: 2px solid var(--sl-color);
    background: #ffffff;
  }
  /* Centered on the outer corners of the 2 px outline. */
  .sl-selection__handle--tl {
    top: -6px;
    left: -6px;
  }
  .sl-selection__handle--tr {
    top: -6px;
    right: -6px;
  }
  .sl-selection__handle--bl {
    bottom: -6px;
    left: -6px;
  }
  .sl-selection__handle--br {
    right: -6px;
    bottom: -6px;
  }

  .sl-selection__label {
    position: absolute;
    bottom: calc(100% + 8px);
    left: -2px;
    color: var(--sl-color);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    font-weight: 400;
    line-height: 16px;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .sl-selection {
      transition: none;
    }
  }
</style>
