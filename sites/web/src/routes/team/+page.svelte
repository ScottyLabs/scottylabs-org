<script lang="ts">
  import { ArrowRight } from "@lucide/svelte";
  import {
    COMMITTEES,
    type Committee,
    type PersonId,
    type TeamMember,
    YEARS,
  } from "#lib/data/team.ts";
  import type { CardData } from "#lib/sections/team/cards.ts";
  import Cards from "#lib/sections/team/Cards.svelte";
  import {
    COMMITTEE_LABELS,
    directorRoleFor,
    personById,
    photoUrlFor,
  } from "#lib/sections/team/lookups.ts";

  const directorCard = (id: PersonId, committee: Committee): CardData => {
    const p = personById(id);
    return {
      name: p.name,
      role: directorRoleFor(committee),
      photoUrl: photoUrlFor(id),
      href: p.linkedin,
    };
  };

  const memberCard = (m: TeamMember): CardData => {
    const p = personById(m.person);
    return {
      name: p.name,
      role: m.role,
      photoUrl: photoUrlFor(m.person),
      href: p.linkedin,
    };
  };

  // Exact colors from Figma with 60% opacity
  const lineColors = [
    // Column 0 - Director of ScottyLabs (transparent)
    "rgba(177, 179, 233, 0.00)",
    // Column 1 - Events
    "rgba(177, 179, 233, 0.60)",
    // Column 2 - Outreach
    "rgba(177, 202, 233, 0.60)",
    // Column 3 - Tech
    "rgba(236, 151, 152, 0.60)",
    // Column 4 - Labrador
    "rgba(232, 187, 174, 0.60)",
    // Column 5 - Design
    "rgba(205, 179, 210, 0.60)",
    // Column 6 - Finance
    "rgba(179, 213, 224, 0.60)",
  ];
</script>

<section class="team-page">
  <h1 style="font-size: 32px; margin: 0 0 24px 0; color: #07123b">Leadership</h1>
  <div
    class="team-chart"
    style="border-radius: 20px; border: 1px solid #B0B0B0; background: #F2F3F7; padding: 48px; padding-left: 130px; padding-right: 40px; position: relative; overflow-x: auto; overflow-y: hidden"
  >
    <!-- Content with relative positioning -->
    <div style="position: relative; z-index: 1; min-width: 1000px">
      <!-- Vertical lines spanning the entire height -->
      <div
        style="position: absolute; inset: 0; display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 12px; pointer-events: none; z-index: 0"
      >
        {#each COMMITTEES as committee, idx (committee)}
          <div style="display: flex; justify-content: center; align-items: flex-start">
            <div
              style="width: 25px; height: 100%; border-radius: 9999px"
              style:background-color={lineColors[idx]}
            ></div>
          </div>
        {/each}
      </div>
      {#each YEARS as year, yearIdx (year.label)}
        <div
          style="position: relative"
          style:margin-bottom={yearIdx === YEARS.length - 1 ? "0" : "60px"}
        >
          <div
            style="display: grid; grid-template-columns: repeat(7, 1fr); gap: 12px; margin-bottom: 16px"
          >
            <div
              style="position: absolute; left: -110px; top: 100px; transform: translateY(-50%); color: #000; font-family: Satoshi, sans-serif; font-size: 20px; font-weight: 700; line-height: 1; white-space: nowrap; z-index: 2"
            >
              {year.label}
            </div>

            <div
              style="position: absolute; top: 100px; left: -20px; width: calc(100% / 7 * 6 + (100% / 7 + 12px) / 2 + 20px); height: 1px; background-color: #819AFF; z-index: 0"
            >
              <div
                style="position: absolute; left: -4px; top: -4px; width: 8px; height: 8px; border-radius: 50%; background-color: #819AFF"
              ></div>
            </div>

            {#each COMMITTEES as committee (committee)}
              {@const id = year.directors[committee]}
              <div style="display: flex; justify-content: center; position: relative; z-index: 1">
                {#if id}
                  <Cards variant="director" person={directorCard(id, committee)} />
                {/if}
              </div>
            {/each}
          </div>

          <div style="display: grid; grid-template-columns: repeat(7, 1fr); gap: 12px">
            {#each COMMITTEES as committee (committee)}
              {@const team = year.teams[committee] ?? []}
              <div
                style="display: grid; grid-template-columns: repeat(2, minmax(0,1fr)); gap: 8px; align-items: center; height: fit-content"
              >
                {#each team as member (`${member.person}-${member.role}`)}
                  <Cards variant="member" person={memberCard(member)} />
                {/each}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
  <div class="team-list">
    {#each YEARS as year, yearIdx (year.label)}
      <details class="team-list__year" open={yearIdx === 0}>
        <summary class="team-list__year-label">{year.label}</summary>
        <div class="team-list__committees">
          {#each COMMITTEES as committee (committee)}
            {@const id = year.directors[committee]}
            {@const team = year.teams[committee] ?? []}
            {#if id !== undefined || team.length > 0}
              <div class="team-list__committee">
                <h3 class="team-list__committee-label">{COMMITTEE_LABELS[committee]}</h3>
                {#if id !== undefined}
                  <div class="team-list__director">
                    <Cards variant="director" person={directorCard(id, committee)} />
                  </div>
                {/if}
                {#if team.length > 0}
                  <div class="team-list__members">
                    {#each team as member (`${member.person}-${member.role}`)}
                      <Cards variant="member" person={memberCard(member)} />
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
          {/each}
        </div>
      </details>
    {/each}
  </div>

  <div
    style="margin-top: 32px; text-align: center; font-family: Satoshi, sans-serif; font-size: 14px; color: #6b7280"
  >
    <a
      href="/over-the-years/"
      style="color: #819AFF; text-decoration: none; display: inline-flex; align-items: center; gap: 6px"
    >
      ScottyLabs Over the Years
      <ArrowRight size={16} />
    </a>
  </div>
</section>

<style>
  .team-page {
    padding: 48px 24px;
    max-width: 1400px;
    margin: 0 auto;
    @media (max-width: 768px) {
      padding: 32px 20px;
    }
  }
  .team-list {
    display: none;
  }
  @media (max-width: 768px) {
    .team-chart {
      display: none;
    }
    .team-list {
      display: flex;
      flex-direction: column;
      gap: 24px;
    }
  }
  .team-list__year-label {
    font-size: 20px;
    font-weight: 700;
    color: #000;
    cursor: pointer;
  }
  .team-list__committees {
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }
  .team-list__committee {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .team-list__committee-label {
    font-size: 16px;
    font-weight: 700;
    color: #07123b;
  }
  .team-list__director {
    width: 160px;
  }
  .team-list__members {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    & :global(.name-member) {
      font-size: 12px;
    }
    & :global(.role-member) {
      font-size: 10px;
    }
  }
</style>
