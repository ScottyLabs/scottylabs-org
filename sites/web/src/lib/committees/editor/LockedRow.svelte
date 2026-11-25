<script lang="ts">
  // A setting that comes from the ScottyLabs theme and cannot be changed here (spec CP-6).
  import Lock from "@lucide/svelte/icons/lock";

  let { label, value, swatch }: { label: string; value: string; swatch?: string } = $props();
</script>

<div class="locked">
  {#if swatch}<span class="locked__swatch" style:background={swatch} aria-hidden="true"></span>{/if}
  <p class="locked__text">
    <span class="locked__label">{label}</span>
    <span class="locked__value">{value}</span>
  </p>
  <Lock class="locked__lock" size={16} strokeWidth={1.75} aria-hidden="true" />
  <span class="p-sr-only">, set by the ScottyLabs theme</span>
</div>

<style>
  .locked {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
  }
  .locked__swatch {
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: 6px;
  }
  .locked__text {
    display: flex;
    flex: 1;
    flex-direction: column;
    margin: 0;
  }
  .locked__label {
    color: var(--p-text-secondary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .locked__value {
    color: var(--p-text);
    font-size: 0.9375rem;
    line-height: 22px;
  }
  .locked :global(.locked__lock) {
    flex: none;
    color: var(--p-text-secondary);
  }
</style>
