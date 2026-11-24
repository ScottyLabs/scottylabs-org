<script lang="ts">
  // A grey placeholder the size of content that is still loading, so nothing jumps when it lands.
  let {
    width = "100%",
    height = "1em",
    radius = "6px",
    inline = false,
    class: className,
  }: {
    width?: string;
    height?: string;
    radius?: string;
    inline?: boolean;
    class?: string;
  } = $props();
</script>

<span
  class={["p-skeleton", inline && "p-skeleton--inline", className]}
  style:width
  style:height
  style:border-radius={radius}
  aria-hidden="true"
></span>

<style>
  .p-skeleton {
    display: block;
    max-width: 100%;
    background: linear-gradient(90deg, #ececec 0%, #f6f6f6 40%, #ececec 80%) 0 0 / 300% 100%;
    animation: shimmer 1.6s ease-in-out infinite;
  }
  .p-skeleton--inline {
    display: inline-block;
    vertical-align: middle;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .p-skeleton {
      animation: none;
      background: #ececec;
    }
  }
</style>
