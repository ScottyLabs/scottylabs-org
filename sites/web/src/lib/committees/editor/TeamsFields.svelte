<script lang="ts">
  // The teams in a Teams block, each with a name, a line about it and its Slack channel.
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowUp from "@lucide/svelte/icons/arrow-up";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import type { BlockByType } from "../../api/types.ts";
  import { Button, IconButton } from "../../portal/ui/index.ts";
  import { LIMITS, cleanChannel } from "../limits.ts";
  import TextField from "./TextField.svelte";

  type Team = BlockByType["teams"]["teams"][number];

  let {
    teams,
    problems,
    onchange,
  }: {
    teams: readonly Team[];
    problems: Readonly<Record<string, string>>;
    onchange: (teams: Team[], key: string | null) => void;
  } = $props();

  function edit(index: number, change: Partial<Team>, field: string) {
    const current = teams[index];
    if (current === undefined) return;
    onchange(teams.with(index, { ...current, ...change }), `teams:${index}:${field}`);
  }

  function move(index: number, step: -1 | 1) {
    const next = [...teams];
    const [team] = next.splice(index, 1);
    if (team === undefined) return;
    next.splice(index + step, 0, team);
    onchange(next, null);
  }
</script>

<fieldset class="teams-fields">
  <legend class="teams-fields__legend">Teams</legend>
  {#if problems.teams}<p class="teams-fields__problem" role="alert">{problems.teams}</p>{/if}
  <ol class="teams-fields__list">
    {#each teams as team, index (index)}
      <li class="teams-fields__team">
        <div class="teams-fields__head">
          <span class="teams-fields__number">Team {index + 1}</span>
          <IconButton size="sm" label={`Move team ${index + 1} up`} icon={ArrowUp} disabled={index === 0} onclick={() => move(index, -1)} />
          <IconButton
            size="sm"
            label={`Move team ${index + 1} down`}
            icon={ArrowDown}
            disabled={index === teams.length - 1}
            onclick={() => move(index, 1)}
          />
          <IconButton size="sm" label={`Remove team ${index + 1}`} icon={X} onclick={() => onchange(teams.filter((_, i) => i !== index), null)} />
        </div>
        <TextField
          label="Name"
          value={team.name}
          max={LIMITS.teamName}
          error={problems[`teams.${index}.name`]}
          onchange={(name) => edit(index, { name }, "name")}
        />
        <TextField
          label="What the team does"
          value={team.about}
          max={LIMITS.teamAbout}
          multiline
          error={problems[`teams.${index}.about`]}
          onchange={(about) => edit(index, { about }, "about")}
        />
        <TextField
          label="Slack channel"
          value={team.channel}
          max={LIMITS.channel}
          prefix="#"
          optional
          placeholder="tech-maps"
          error={problems[`teams.${index}.channel`]}
          onchange={(channel) => edit(index, { channel: cleanChannel(channel) }, "channel")}
        />
      </li>
    {/each}
  </ol>
  <div class="teams-fields__add">
    <Button
      variant="outline"
      size="sm"
      icon={Plus}
      disabled={teams.length >= LIMITS.maxTeams}
      onclick={() => onchange([...teams, { name: "", about: "", channel: "" }], null)}
    >
      Add a team
    </Button>
    <span class="teams-fields__count">{teams.length} of {LIMITS.maxTeams}</span>
  </div>
</fieldset>

<style>
  .teams-fields {
    margin: 0;
    padding: 0;
    border: 0;
  }
  .teams-fields__legend {
    margin-bottom: 8px;
    padding: 0;
    color: var(--p-text);
    font-size: 0.875rem;
    font-weight: 500;
    line-height: 20px;
  }
  .teams-fields__problem {
    margin: 0 0 8px;
    color: var(--p-danger-text);
    font-size: 0.8125rem;
    line-height: 18px;
  }
  .teams-fields__list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0 0 12px;
    padding: 0;
    list-style: none;
  }
  .teams-fields__team {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius);
  }
  .teams-fields__head {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .teams-fields__number {
    flex: 1;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    font-weight: 500;
  }
  .teams-fields__add {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .teams-fields__count {
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
  }
</style>
