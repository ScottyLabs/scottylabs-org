<script lang="ts">
  let { size = 16, label }: { size?: number; label?: string } = $props();
</script>

<span
  class="spinner"
  style:--size="{size}px"
  role={label === undefined ? undefined : "status"}
  aria-hidden={label === undefined ? "true" : undefined}
>
  {#if label !== undefined}<span class="p-sr-only">{label}</span>{/if}
</span>

<style>
  .spinner {
    display: inline-block;
    flex: none;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 2s;
    }
  }
</style>
