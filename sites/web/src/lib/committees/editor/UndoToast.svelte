<script lang="ts" module>
  export interface UndoNotice {
    id: number;
    message: string;
  }
</script>

<script lang="ts">
  // "Stats moved above What we do. Undo" (Figma 109:7854). Stays while hovered or focused.
  let { notice, onundo, ondismiss }: { notice: UndoNotice | null; onundo: () => void; ondismiss: () => void } =
    $props();

  const DURATION_MS = 6000;
  let paused = $state(false);

  $effect(() => {
    if (notice === null || paused) return;
    const timer = setTimeout(ondismiss, DURATION_MS);
    return () => clearTimeout(timer);
  });
</script>

<div class="undo-region" role="status" aria-live="polite">
  {#if notice !== null}
    {#key notice.id}
      <div
        class="undo"
        role="group"
        aria-label="Change made"
        onpointerenter={() => (paused = true)}
        onpointerleave={() => (paused = false)}
        onfocusin={() => (paused = true)}
        onfocusout={() => (paused = false)}
      >
        <span>{notice.message}</span>
        <button type="button" class="undo__button" onclick={onundo}>Undo</button>
      </div>
    {/key}
  {/if}
</div>

<style>
  .undo-region {
    position: fixed;
    bottom: 24px;
    left: 50%;
    z-index: 70;
    transform: translateX(-50%);
  }
  .undo {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    border-radius: var(--p-radius-pill);
    background: var(--p-toast);
    color: #ffffff;
    font-size: 0.9375rem;
    line-height: 22px;
    white-space: nowrap;
    box-shadow: var(--p-shadow-pop);
  }
  .undo__button {
    color: #ffffff;
    font-weight: 500;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;

    &:focus-visible {
      border-radius: 2px;
      outline: 2px solid #ffffff;
      outline-offset: 2px;
    }
  }
</style>
