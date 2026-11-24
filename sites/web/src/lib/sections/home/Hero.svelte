<script lang="ts">
  import Button from "#lib/components/Button.svelte";
  import Ticker from "#lib/ticker/Ticker.svelte";
  // inlined so the background never flashes blank
  import heroBg from "#lib/assets/hero-bg.svg?inline";
</script>

<section class="hero-container">
  <div class="centered-section centered-section--relative">
    <div class="hero-section">
      <div class="title-container">
        <span class="title">
          ScottyLabs<div class="decoration">
            <div class="decoration__line"></div>
            <div class="decoration__line"></div>
            <div class="decoration__line"></div>
            <div class="decoration__line"></div>
            <div class="decoration__tooltip">
              font-family: Satoshi; <br />
              font-size: 3.75rem;
              <br />
              font-weight: 700;
              <div class="decoration__tooltip__arrow"></div>
            </div>
          </div>
        </span>
      </div>
      <div class="subtitle">The best place to build software @ CMU</div>
      <div class="marketing-text">
        We&rsquo;re a student-run organization dedicated to building tech that enhances campus
        life&mdash;empowering the Carnegie Mellon University community to create, collaborate, and
        solve real-world problems through apps and events.
      </div>

      <div class="action-buttons">
        <a href="https://tartanconnect.cmu.edu/scottylabs/club_signup" target="_blank">
          <Button label="Join our mailing list" variant="primary" />
        </a>
        <a href="mailto:hello@scottylabs.org">
          <Button label="Reach out" variant="outlined" />
        </a>
      </div>

      <link rel="preload" href={heroBg} as="image" />
    </div>
    <div class="hero-background">
      <img src={heroBg} alt="" fetchpriority="high" />
    </div>
  </div>
  <!-- Announcements from the portal, between the strip's two hairlines. -->
  <div class="ticker-slot">
    <Ticker />
  </div>
</section>

<style>
  .hero-container {
    overflow: hidden;
    /* Clip without scrolling, so focusing an announcement in the ticker cannot shift the hero. */
    overflow: clip;
    position: relative;
    margin-bottom: -60px;
  }
  .hero-section {
    padding-top: 7%;
    height: 700px;
    box-sizing: border-box;
    position: relative;
  }
  .hero-background {
    position: absolute;
    left: 45%;
    top: 0px;
    bottom: 100px;
    /* basically prevents the image from being draggable, since it's now hidden behind the text */
    z-index: -1;
    & > img {
      height: 100%;
      object-fit: cover;
      object-position: top left;
    }
  }
  @media (max-width: 1024px) {
    /* The ticker sits between the text and the art, so the art's wrapper steps aside. */
    .hero-container {
      display: flex;
      flex-direction: column;
      margin-bottom: 0;
    }
    .centered-section--relative {
      display: contents;
    }
    .hero-section {
      order: 1;
      height: auto;
      padding-top: 60px;
      padding-inline: var(--padding);
    }
    .hero-background {
      order: 3;
      position: static;
      height: clamp(240px, 45vw, 420px);
      margin-top: 20px;
      overflow: hidden;
    }
  }
  .title {
    font-weight: 700;
    font-size: var(--text-6xl);
    color: var(--black-900);
    position: relative;
    content: 'If you want to build the next generation of production-grade products for CMU students and meet other passionate coders like yourself, we would love to have you! And since you found this, email "ilovescottylabs" to hello@scottylabs.org for a special gift!';
  }
  .decoration {
    position: absolute;
    z-index: -1;
    inset: 14px -1px 10px -1px;
  }
  .decoration__line {
    position: absolute;
    background-color: var(--black-200);
    &:nth-child(1) {
      inset: -200px 0 -100px auto;
      width: 1px;
    }
    &:nth-child(2) {
      inset: -200px auto -200px 0;
      width: 1px;
    }
    &:nth-child(3) {
      inset: auto -300px 0 -200px;
      height: 1px;
    }
    &:nth-child(4) {
      inset: 0 -300px auto -200px;
      height: 1px;
    }
  }

  .decoration__tooltip {
    position: absolute;
    top: 0;
    left: 80px;
    width: max-content;

    transform: translate(0, calc(-100% - 12px));
    color: var(--black-300);
    font-size: var(--text-xs);
    padding: 6px 16px;
    border: 1px solid var(--black-200);
    font-weight: 300;
    font-family: "JetBrainsMono";
  }
  .decoration__tooltip__arrow {
    position: absolute;
    bottom: 0;
    left: 60%;
    transform: translate(0, 50%) rotate(-45deg);
    width: 16px;
    height: 16px;

    border: 1px solid var(--black-200);
    clip-path: polygon(0% 0%, 0% 100%, 100% 100%);
    background-color: var(--bg-color);
  }
  .subtitle {
    font-weight: 700;
    font-size: var(--text-3xl);
    color: var(--black-900);
    margin-top: 10px;
  }
  .marketing-text {
    margin-top: 10px;
    font-size: var(--text-xl);
    color: var(--black-500);
    width: 60%;
    @media (max-width: 1024px) {
      width: auto;
    }
  }
  .action-buttons {
    margin-top: 60px;
    display: flex;
    gap: 20px;
    flex-wrap: wrap;
    @media (max-width: 768px) {
      margin-top: 40px;
    }
  }

  .ticker-slot {
    position: absolute;
    bottom: 110px;
    left: 0;
    right: 0;
    backdrop-filter: blur(2px);
    @media (max-width: 1024px) {
      order: 2;
      position: static;
      margin-top: 40px;
    }
  }
  /* With nothing to announce, the strip's two hairlines become one. */
  .ticker-slot:not(:has(:global(.ticker))) {
    border-top: 1px solid var(--p-border);
    @media (max-width: 1024px) {
      margin-top: 20px;
    }
  }
</style>
