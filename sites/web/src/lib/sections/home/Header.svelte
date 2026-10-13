<script lang="ts">
  import { page } from "$app/state";
  import scottylabsLogo from "#lib/assets/scottylabs-logo.svg";
  import teamIcon from "#lib/assets/icons/team.svg";
  import sponsorsIcon from "#lib/assets/icons/sponsors.svg";

  const navLinks = [
    {
      icon: teamIcon,
      url: "/team/",
      text: "Team",
    },
    {
      icon: sponsorsIcon,
      url: "/sponsors/",
      text: "Sponsors",
    },
  ];
</script>

<header class="main-header-container">
  <div class="main-header-layout">
    <a class="logo" href="/">
      <img class="logo__img" src={scottylabsLogo} alt="" />

      <div class="logo__text">ScottyLabs</div>
    </a>
    <nav class="main-nav">
      {#each navLinks as { icon, url, text } (url)}
        <a
          class={["nav-button", page.url.pathname.startsWith(url) && "nav-button--active"]}
          href={url}
        >
          <img class="nav-button__icon" src={icon} alt="" />
          <div class="nav-button__text">{text}</div>
        </a>
      {/each}
    </nav>
  </div>
</header>

<style>
  .main-header-container {
    position: sticky;
    top: 0;
    backdrop-filter: blur(40px);
    background: rgba(255, 255, 255, 0.809);
    z-index: 999;
  }
  .main-header-layout {
    max-width: 1400px;
    margin-inline: auto;
    padding: 20px 40px;
    @media (max-width: 768px) {
      padding: 14px 20px;
    }
    display: flex;
  }

  .logo {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: var(--text-xl);
    font-weight: 500;
    margin-right: auto;
  }

  .logo__img {
    height: 1.5em;
  }
  .logo__text {
    flex-grow: 0;
    color: var(--black-900);
  }
  .main-nav {
    display: flex;
    gap: 2px;
  }
  .nav-button {
    display: flex;
    gap: 4px;
    padding: 10px 13px;

    font-size: var(--text-base);
    align-items: center;
    color: var(--black-900);
    border: 1px solid transparent;
    border-radius: var(--border-radius-base);

    &:hover {
      background-color: var(--black-50);
    }
  }

  .nav-button--active {
    background-color: var(--blue-50);
    border-color: var(--black-200);
  }
  .nav-button__icon {
    height: 1.1em;
  }
</style>
