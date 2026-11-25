<script lang="ts">
  // People and their roles, one row per role (Figma 107:5980 for admins, 111:8915 for a
  // director's team). The role button opens the actions the viewer may take on that role.
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { MediaQuery } from "svelte/reactivity";
  import type { Role } from "#lib/api/types.ts";
  import { committeeName } from "#lib/portal/committees.ts";
  import { Avatar, CommitteeChip, Menu, type MenuItem } from "#lib/portal/ui/index.ts";
  import {
    type AssignmentRow,
    type ProjectNames,
    ROLE_LABELS,
    appointedText,
    canEditText,
  } from "./people.ts";

  let {
    rows,
    view,
    projectNames,
    viewerId,
    now,
    highlight = null,
    canManage,
    onchange,
    onedit,
    onremove,
  }: {
    rows: readonly AssignmentRow[];
    view: "admin" | "director";
    projectNames: ProjectNames | null;
    viewerId: string;
    now: Date;
    /** The role that was just added or changed. */
    highlight?: string | null;
    canManage: (row: AssignmentRow) => boolean;
    onchange: (row: AssignmentRow, role: Role) => void;
    onedit: (row: AssignmentRow) => void;
    onremove: (row: AssignmentRow) => void;
  } = $props();

  const ROLES: readonly Role[] = ["lead", "director", "admin"];

  // Stacked rows on phones put the role button at the right edge, so its menu opens leftwards.
  // Matches the breakpoint of p-table--stack.
  const stacked = new MediaQuery("(max-width: 720px)", false);

  let body = $state<HTMLTableSectionElement | null>(null);

  /**
   * Focuses the role button of the row at `index`, or of the nearest row above it, so keyboard
   * users carry on from where a removed row was. False when no row has a role button.
   */
  export function focusRoleNear(index: number): boolean {
    const rowElements = Array.from(body?.rows ?? []);
    const nearest = [...rowElements.slice(index), ...rowElements.slice(0, index).toReversed()];
    for (const row of nearest) {
      const button = row.querySelector<HTMLButtonElement>(".people-table__role");
      if (button !== null) {
        button.focus();
        return true;
      }
    }
    return false;
  }

  function menuItems(row: AssignmentRow): (MenuItem | "separator")[] {
    const { role, committee } = row.assignment;
    const items: (MenuItem | "separator")[] = [];
    if (view === "admin") {
      for (const other of ROLES) {
        if (other !== role) items.push({ label: `Make ${other}`, onSelect: () => onchange(row, other) });
      }
    }
    if (role === "lead") items.push({ label: "Edit projects and ticker", onSelect: () => onedit(row) });
    items.push("separator", {
      label: view === "director" && committee !== null ? `Remove from ${committeeName(committee)}` : "Remove role",
      danger: true,
      onSelect: () => onremove(row),
    });
    return items;
  }
</script>

<div class="p-table-card people-table-card">
  <table class="p-table p-table--stack people-table">
    <thead>
      <tr>
        <th scope="col">Person</th>
        {#if view === "admin"}<th scope="col">Committee</th>{/if}
        <th scope="col">Role</th>
        <th scope="col">Can edit</th>
        {#if view === "director"}<th scope="col">Appointed</th>{/if}
      </tr>
    </thead>
    <tbody bind:this={body}>
      {#each rows as row (row.assignment.id)}
        {@const { person, assignment } = row}
        <tr class={[highlight === assignment.id && "p-table__row--highlight"]}>
          <td class="p-table__cell--full">
            <span class="people-table__person">
              <Avatar initials={person.initials} committee={assignment.committee} />
              <span class="people-table__name">
                <span class="people-table__full-name">
                  {person.name}{#if row.isMe}<span class="people-table__you">you</span>{/if}
                </span>
                {#if !person.hasSignedIn}
                  <span class="people-table__pending">Not signed in yet</span>
                {/if}
              </span>
            </span>
          </td>
          {#if view === "admin"}
            <td data-label="Committee">
              <CommitteeChip committee={assignment.committee ?? "club"} />
            </td>
          {/if}
          <td data-label="Role">
            {#if canManage(row)}
              <Menu
                items={menuItems(row)}
                label={`Role actions for ${person.name}`}
                align={stacked.current ? "end" : "start"}
                width={232}
              >
                {#snippet trigger(props)}
                  <button
                    {...props}
                    type="button"
                    class="people-table__role"
                    aria-label={`${ROLE_LABELS[assignment.role]}. Change ${person.name}\u2019s role`}
                  >
                    {ROLE_LABELS[assignment.role]}
                    <ChevronDown size={14} strokeWidth={2} aria-hidden="true" />
                  </button>
                {/snippet}
              </Menu>
            {:else}
              <span class="people-table__role-text">{ROLE_LABELS[assignment.role]}</span>
            {/if}
          </td>
          <td data-label="Can edit" class="p-table__muted people-table__edits">
            {canEditText(assignment, projectNames)}
          </td>
          {#if view === "director"}
            <td data-label="Appointed" class="p-table__muted people-table__appointed">
              {appointedText(assignment, viewerId, now)}
            </td>
          {/if}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  /* The role menu opens below its button, so the card must not clip it, as a short team list
     would. The stacked layout on phones never scrolls sideways. The last row rounds its own
     corners for the hover and highlight backgrounds. */
  .people-table-card {
    overflow: visible;
  }
  .people-table tbody tr:last-child td:first-child {
    border-bottom-left-radius: var(--p-radius-xl);
  }
  .people-table tbody tr:last-child td:last-child {
    border-bottom-right-radius: var(--p-radius-xl);
  }
  @media (max-width: 720px) {
    .people-table tbody tr:last-child {
      border-radius: 0 0 var(--p-radius-xl) var(--p-radius-xl);
    }
  }
  .people-table td {
    font-size: 0.75rem;
    line-height: 16px;
  }
  .people-table__person {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .people-table__name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .people-table__full-name {
    color: var(--p-text);
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .people-table__you {
    margin-left: 10px;
    color: var(--p-text-tertiary);
    font-weight: 400;
  }
  .people-table__pending {
    color: var(--p-text-tertiary);
  }
  .people-table__role {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--p-border);
    border-radius: 6px;
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-size: 0.75rem;
    font-weight: 500;
    line-height: 16px;
    cursor: pointer;

    &:hover {
      background: var(--p-surface-muted);
    }
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 2px;
    }
  }
  .people-table__role-text {
    color: var(--p-text);
    font-size: 0.875rem;
  }
  .people-table__edits,
  .people-table__appointed {
    max-width: 260px;
  }
  @media (max-width: 720px) {
    .people-table__edits,
    .people-table__appointed {
      max-width: none;
    }
  }
</style>
