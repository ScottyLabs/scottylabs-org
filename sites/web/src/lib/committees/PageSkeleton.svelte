<script lang="ts">
  // Holds the shape of a committee page while it loads, so nothing jumps when it arrives.
  import Skeleton from "../portal/ui/Skeleton.svelte";
  import type { CommitteeSlug } from "../api/types.ts";

  let { name, slug }: { name: string; slug: CommitteeSlug } = $props();
</script>

<div class="skeleton" aria-busy="true">
  <p class="cb-sr-only" role="status">Loading the {name} page.</p>
  <div class="skeleton__crumbs" aria-hidden="true"><Skeleton width="140px" height="20px" /></div>
  <div class="skeleton__title" aria-hidden="true">
    <p class="cb-inspector">[committee="{slug}"]</p>
    <p class="skeleton__name">{name}</p>
    <Skeleton width="min(420px, 80%)" height="28px" />
  </div>
  <div class="skeleton__cover" aria-hidden="true"><Skeleton width="100%" height="100%" radius="20px" /></div>
  <div class="skeleton__row" aria-hidden="true">
    <div class="skeleton__text">
      <Skeleton width="40%" height="34px" />
      <Skeleton width="100%" height="18px" />
      <Skeleton width="92%" height="18px" />
      <Skeleton width="70%" height="18px" />
    </div>
    <Skeleton width="100%" height="220px" radius="16px" />
  </div>
</div>

<style>
  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 40px;
  }
  .skeleton__title {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .skeleton__name {
    margin: 0;
    color: var(--cb-text);
    font-size: 5rem;
    font-weight: 600;
    line-height: 84px;
    letter-spacing: -0.025em;
  }
  .skeleton__cover {
    aspect-ratio: 1200 / 520;
  }
  .skeleton__row {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 24px;
    margin-top: 32px;
  }
  .skeleton__text {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  @media (max-width: 640px) {
    .skeleton {
      gap: 24px;
    }
    .skeleton__name {
      font-size: 3rem;
      line-height: 52px;
    }
    .skeleton__cover {
      aspect-ratio: 280 / 175;
    }
    .skeleton__row {
      grid-template-columns: 1fr;
    }
  }
</style>
