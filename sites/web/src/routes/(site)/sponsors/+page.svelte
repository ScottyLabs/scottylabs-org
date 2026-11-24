<script lang="ts">
  import SponsorSection from "#lib/components/SponsorSection.svelte";

  const sponsorPacketURL = "https://assets.scottylabs.org/legal/sponsorship-packet-2627.pdf";
</script>

<section class="centered-section">
  <h1 class="header">Sponsors</h1>
  <p class="description">
    Interested in becoming a sponsor? View our
    <a href={sponsorPacketURL} target="_blank" rel="noreferrer">sponsorship packet</a>
    or contact us at <a href="mailto:sponsorship@scottylabs.org">sponsorship@scottylabs.org</a>.
  </p>
  <SponsorSection />
</section>

<style>
  .header {
    font-size: 2.25rem;
    color: var(--black-900);
    margin-top: 50px;
  }
  .description {
    font-size: 1.25rem;
    color: var(--black-900);
    margin-top: 15px;
    overflow-wrap: anywhere;
    a {
      text-decoration: underline;
      text-underline-offset: 3px;
    }
  }
</style>
