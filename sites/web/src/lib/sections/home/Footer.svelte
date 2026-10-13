<script lang="ts">
  import tartanConnectIcon from "#lib/assets/icons/socials/tartan-connect.png";
  import instagramIcon from "#lib/assets/icons/socials/instagram.svg";
  import linkedinIcon from "#lib/assets/icons/socials/linkedin.svg";
  import discordIcon from "#lib/assets/icons/socials/discord.svg";
  import slackIcon from "#lib/assets/icons/socials/slack.svg";
  import divider from "#lib/assets/icons/socials/social-divider.png";

  const socialLinks = [
    {
      icon: tartanConnectIcon,
      name: "TartanConnect",
      url: "https://tartanconnect.cmu.edu/scottylabs/club_signup",
      color: "hsl(0,100%,30%)",
    },
    {
      icon: instagramIcon,
      name: "Instagram",
      url: "https://www.instagram.com/cmu.scottylabs/",
      color: "#6532D2",
    },
    {
      icon: linkedinIcon,
      name: "LinkedIn",
      url: "https://www.linkedin.com/company/scottylabs",
      color: "#0E76A8",
    },
    {
      icon: discordIcon,
      name: "Discord",
      url: "https://go.scottylabs.org/discord",
      color: "#5865F2",
    },
    {
      icon: slackIcon,
      name: "Slack",
      url: "https://go.scottylabs.org/slack",
      color: "#4A154B",
    },
  ];

  const legalDocs = [
    {
      label: "IRS determination letter",
      url: "https://assets.scottylabs.org/legal/irs-determination-letter.pdf",
    },
    {
      label: "Articles of incorporation",
      url: "https://assets.scottylabs.org/legal/articles-of-incorporation.pdf",
    },
    {
      label: "Form 1023-EZ",
      url: "https://assets.scottylabs.org/legal/form-1023-ez.pdf",
    },
  ];
  const CURRENT_YEAR = new Date().getFullYear();
</script>

<footer class="footer-container">
  <div class="footer">
    <div class="footer__copyright">Made with ❤️ by ScottyLabs © {CURRENT_YEAR}</div>
    {#each socialLinks as { icon, name, url, color }, i (name)}
      <a
        class="social-link"
        href={url}
        target="_blank"
        rel="noreferrer"
        aria-label={name}
        style:--bg-color={color}
      >
        <div>
          <img src={icon} alt="" />
          <div class="social-link__description">{name}</div>
        </div>
      </a>
      {#if i !== socialLinks.length - 1}
        <img class="social-link-divider" src={divider} alt="" />
      {/if}
    {/each}
  </div>
  <div class="footer__legal">
    <div>ScottyLabs is a 501(c)(3) tax-exempt nonprofit organization.</div>
    <div>502 W 7th St Ste 100, Erie, PA 16502 · EIN 42-2037653 · D-U-N-S 145005627</div>
    <div class="footer__legal-docs">
      {#each legalDocs as { label, url }, i (url)}
        {#if i > 0}{" · "}{/if}
        <a href={url} target="_blank" rel="noreferrer">{label}</a>
      {/each}
    </div>
  </div>
</footer>

<style>
  .footer-container {
    border-top: 8px solid black;
    border-image-source: linear-gradient(90deg, #ff004e 0%, #4f2485 54%, #40d2fc 100%);
    border-image-slice: 1;
  }
  @property --bg-percent {
    syntax: "<length-percentage>";
    initial-value: 0%;
    inherits: false;
  }
  .footer {
    max-width: 1300px;
    height: 130px;
    padding: 0 40px;
    margin: auto;
    display: flex;
    overflow: hidden;
    align-items: stretch;
    @media (max-width: 768px) {
      flex-wrap: wrap;
      height: auto;
      padding: 0 20px;
    }

    & > .footer__copyright {
      color: var(--black-900);
      font-size: 1.25rem;
      margin-right: auto;
      align-self: center;
      @media (max-width: 768px) {
        flex-basis: 100%;
        margin: 20px 0;
        text-align: center;
      }
    }
    & > .social-link {
      --bg-percent: 0%;
      --bg-color: red;
      width: 95px;
      @media (max-width: 768px) {
        flex: 1;
        height: 90px;
      }
      display: flex;
      align-items: center;
      transition: 0.1s --bg-percent ease-out;
      background-image: linear-gradient(
        180deg,
        var(--bg-color) 0%,
        var(--bg-color) var(--bg-percent),
        white var(--bg-percent),
        white calc(100% - var(--bg-percent)),
        var(--bg-color) calc(100% - var(--bg-percent)),
        var(--bg-color) 100%
      );

      & * {
        text-align: center;
        margin: auto;
        transition: 0.1s all;
      }
      & img {
        height: 30px;
      }
      & .social-link__description {
        font-size: 0.875rem;
        color: var(--black-900);
        height: 15px;
        @media (max-width: 768px) {
          display: none;
        }
      }
      &:hover {
        --bg-percent: 50%;
        & img {
          filter: invert(1);
        }
        & .social-link__description {
          color: var(--black-100);
        }
        &:active {
          filter: brightness(80%);
        }
      }
    }
    & > .social-link-divider {
      margin: -25px 0;
      width: 20px;
      @media (max-width: 768px) {
        display: none;
      }
    }
  }
  .footer__legal {
    max-width: 1300px;
    margin: 0 auto;
    padding: 16px 40px 20px;
    @media (max-width: 768px) {
      padding: 16px 20px 20px;
    }
    border-top: 1px solid var(--black-200);
    color: var(--black-500);
    font-size: 0.875rem;
    line-height: 1.6;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 2px;

    & a {
      color: var(--blue-600);
    }
    & a:hover {
      text-decoration: underline;
    }
  }
</style>
