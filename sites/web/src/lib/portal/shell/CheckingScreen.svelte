<script lang="ts">
  // "Signing you in. Checking what you can edit." while GET /api/me is on its way. It waits a
  // moment before appearing so a fast answer does not flash it.
  import { onMount } from "svelte";
  import DogMark from "../ui/DogMark.svelte";

  let { delay = 150 }: { delay?: number } = $props();

  let visible = $state(false);

  onMount(() => {
    const timer = setTimeout(() => (visible = true), delay);
    return () => clearTimeout(timer);
  });
</script>

<svelte:head>
  <title>Signing you in - ScottyLabs Portal</title>
</svelte:head>

<div class="checking" role="status" aria-live="polite">
  {#if visible}
    <div class="checking__content">
      <DogMark variant="gradient" width={76} />
      <p class="checking__title">Signing you in</p>
      <p class="checking__detail">Checking what you can edit.</p>
      <span class="checking__progress" aria-hidden="true"><span></span></span>
    </div>
  {:else}
    <span class="p-sr-only">Checking what you can edit.</span>
  {/if}
</div>

<style>
  .checking {
    display: grid;
    place-items: center;
    min-height: 100vh;
    min-height: 100dvh;
    padding: 24px;
    box-sizing: border-box;
    background: var(--p-surface);
    font-family: var(--p-font);
  }
  .checking__content {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 220px;
    margin-top: -6vh;
    text-align: center;
    animation: fade-in 240ms var(--p-ease);
  }
  .checking__title {
    margin: 18px 0 0;
    color: var(--p-text);
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .checking__detail {
    margin: 18px 0 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }
  .checking__progress {
    position: relative;
    width: 220px;
    height: 4px;
    margin-top: 18px;
    border-radius: 2px;
    background: var(--p-surface-strong);
    overflow: hidden;

    & > span {
      position: absolute;
      inset: 0 auto 0 0;
      width: 64%;
      border-radius: 2px;
      background: var(--p-brand-gradient);
      animation: progress 1.4s var(--p-ease) infinite;
    }
  }
  @keyframes progress {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(160%);
    }
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .checking__content {
      animation: none;
    }
    .checking__progress > span {
      animation: none;
    }
  }
</style>
