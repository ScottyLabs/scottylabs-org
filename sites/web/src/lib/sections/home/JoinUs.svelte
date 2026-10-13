<script lang="ts">
  import { MapPin } from "@lucide/svelte";

  const workSessions = {
    name: "Committee Work Sessions",
    time: "Saturdays, 4-6 PM",
    locations: [
      {
        department: "Tech",
        location: "Tepper 2700, 2701, 2613",
        mapUrl: "https://maps.scottylabs.org/TEP-2700",
      },
      {
        department: "Finance/Events/Outreach",
        location: "Tepper 3808",
        mapUrl: "https://maps.scottylabs.org/TEP-3808",
      },
      {
        department: "Design",
        location: "Tepper 3801",
        mapUrl: "https://maps.scottylabs.org/TEP-3801",
      },
      {
        department: "Foundry",
        location: "Tepper 2612",
        mapUrl: "https://maps.scottylabs.org/TEP-2612",
      },
      {
        department: "Labrador",
        location: "Swartz Center (Tepper 3713)",
        mapUrl: "https://maps.scottylabs.org/TEP-3713",
      },
    ],
  };
</script>

<div class="container">
  <div class="centered-section layout">
    <div class="sessions">
      <h1 class="main-heading">Join Us!</h1>
      <h2 class="sessions-header">{workSessions.name}</h2>
      <p class="sessions-time">{workSessions.time}</p>
      <dl class="locations">
        {#each workSessions.locations as { department, location, mapUrl } (department)}
          <div class="location">
            <dt class="location__department">{department}</dt>
            <dd class="location__room">
              <a href={mapUrl} target="_blank" rel="noreferrer">
                <MapPin size={14} />
                {location}
              </a>
            </dd>
          </div>
        {/each}
      </dl>
    </div>
    <div class="calendar">
      <iframe
        title="ScottyLabs events calendar"
        src="https://calendar.google.com/calendar/embed?src=admin%40scottylabs.org&ctz=America%2FNew_York&mode=week"
        width="0"
        height="0"
      ></iframe>
    </div>
  </div>
</div>

<style>
  .container {
    padding-top: 85px;
    padding-bottom: 85px;
    @media (max-width: 768px) {
      padding-top: 60px;
      padding-bottom: 60px;
    }
  }
  .layout {
    display: grid;
    grid-template-columns: 380px 1fr;
    gap: 60px;
    @media (max-width: 1024px) {
      grid-template-columns: 1fr;
      gap: 40px;
    }
  }
  .main-heading {
    color: var(--black-900);
  }
  .sessions-header {
    margin-top: 40px;
    color: var(--black-900);
    font-size: var(--text-2xl);
    @media (max-width: 1024px) {
      margin-top: 30px;
    }
  }
  .sessions-time {
    margin-top: 4px;
    color: var(--black-500);
    font-style: italic;
  }
  .locations {
    margin: 24px 0 0;
    border: 1px solid var(--black-200);
    border-radius: var(--border-radius-base);
    overflow: hidden;
  }
  .location {
    padding: 14px 18px;
    &:nth-child(even) {
      background-color: var(--black-50);
    }
    & + .location {
      border-top: 1px solid var(--black-200);
    }
  }
  .location__department {
    color: var(--black-900);
    font-weight: 700;
  }
  .location__room {
    margin: 2px 0 0;
    color: var(--black-500);
    font-weight: 400;
    & > a {
      display: inline-flex;
      align-items: baseline;
      gap: 6px;
      color: var(--blue-600);
      &:hover {
        text-decoration: underline;
      }
      & > :global(svg) {
        flex-shrink: 0;
        align-self: center;
      }
    }
  }
  .calendar {
    min-height: 560px;
    border-radius: 10px;
    overflow: hidden;
    outline: 1px solid var(--black-300);
    outline-offset: 0;
    transition: 0.1s all;
    &:hover {
      outline-offset: 10px;
    }
    & > iframe {
      width: 100%;
      height: 100%;
      border: 0;
    }
  }
</style>
