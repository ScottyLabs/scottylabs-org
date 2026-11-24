<script lang="ts">
  // "On the ticker": when the announcement goes live, when it leaves, and the slot it takes.
  import type { Announcement, PersonRef } from "../../api/types.ts";
  import { formatDay } from "../../portal/dates.ts";
  import { listNames, shortDateTime } from "../format.ts";

  let {
    now,
    expiresAt,
    editing,
    direct,
    approvers,
    slots,
  }: {
    now: Date;
    expiresAt: string | null;
    editing: Announcement | null;
    /** Goes live without approval. */
    direct: boolean;
    approvers: readonly PersonRef[];
    slots: { used: number; total: number } | null;
  } = $props();

  const live = $derived(editing !== null && editing.state === "live");

  const goesLive = $derived.by(() => {
    if (live && editing?.startsAt) return `Went live ${formatDay(editing.startsAt, now)}`;
    if (direct) return `Now, ${formatDay(now, now)}`;
    return approvers.length > 0 ? `When ${listNames(approvers.map((person) => person.name))} approves it` : "When it is approved";
  });

  const slotLine = $derived.by(() => {
    if (slots === null) return null;
    if (live) return "It keeps its slot until it leaves.";
    if (!direct) return `${slots.used} of ${slots.total} slots are in use now. It takes the next free one once approved.`;
    if (slots.used >= slots.total) return `All ${slots.total} slots are in use.`;
    return `This takes slot ${slots.used + 1} of ${slots.total}.`;
  });
</script>

<section class="schedule" aria-labelledby="composer-schedule-heading">
  <h2 id="composer-schedule-heading" class="schedule__title">On the ticker</h2>
  <div class="schedule__line" aria-hidden="true">
    <span class="schedule__start"></span>
    <span class="schedule__track"></span>
    <span class="schedule__end"></span>
  </div>
  <dl class="schedule__times">
    <div>
      <dt>Goes live</dt>
      <dd>{goesLive}</dd>
    </div>
    <div class="schedule__leaves">
      <dt>Leaves on its own</dt>
      <dd>{expiresAt === null ? "Not set" : shortDateTime(expiresAt, now)}</dd>
    </div>
  </dl>
  {#if slotLine !== null}<p class="schedule__slots">{slotLine}</p>{/if}
</section>

<style>
  .schedule {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 24px 20px 20px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-xl);
    background: var(--p-surface);
  }
  .schedule__title {
    margin: 0;
    color: var(--p-text);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
  }
  .schedule__line {
    display: flex;
    align-items: center;
    height: 12px;
  }
  .schedule__start,
  .schedule__end {
    box-sizing: border-box;
    width: 10px;
    height: 10px;
    flex: none;
    border-radius: 50%;
  }
  .schedule__start {
    background: var(--p-inverse);
  }
  .schedule__end {
    border: 2px solid var(--p-inverse);
    background: var(--p-surface);
  }
  .schedule__track {
    flex: 1;
    height: 2px;
    background: var(--p-brand-gradient);
  }
  .schedule__times {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    margin: 0;

    & div {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
    & dt {
      color: var(--p-text-tertiary);
      font-size: 0.75rem;
      line-height: 16px;
    }
    & dd {
      margin: 0;
      color: var(--p-text);
      font-size: 0.875rem;
      line-height: 20px;
    }
  }
  .schedule__leaves {
    text-align: right;
  }
  .schedule__slots {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
    color: var(--p-text);
    font-size: 0.875rem;
    line-height: 20px;
  }
</style>
