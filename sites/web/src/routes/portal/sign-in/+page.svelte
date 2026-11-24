<script lang="ts">
  // Sign in through ScottyLabs single sign-on (Keycloak), or as a sample person where the service
  // allows development sign-in. People who are already signed in go straight to returnTo.
  import { onMount } from "svelte";
  import LogIn from "@lucide/svelte/icons/log-in";
  import Megaphone from "@lucide/svelte/icons/megaphone";
  import SquareKanban from "@lucide/svelte/icons/square-kanban";
  import Users from "@lucide/svelte/icons/users";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import heroArt from "#lib/assets/hero-bg.svg";
  import { apiUrl, get, isApiError } from "#lib/api/client.ts";
  import type { AuthConfigResponse } from "#lib/api/types.ts";
  import { safeReturnTo } from "#lib/portal/paths.ts";
  import { getPortalSession } from "#lib/portal/session.svelte.ts";
  import DevSignIn from "#lib/portal/sign-in/DevSignIn.svelte";
  import { signInNotice } from "#lib/portal/sign-in-messages.ts";
  import CheckingScreen from "#lib/portal/shell/CheckingScreen.svelte";
  import { Button, Callout, DogMark } from "#lib/portal/ui/index.ts";

  const session = getPortalSession();

  const returnTo = $derived(safeReturnTo(page.url.searchParams.get("returnTo")));
  const notice = $derived(signInNotice(page.url.searchParams));
  const loginHref = $derived(apiUrl("/auth/login", { returnTo }));

  let config = $state.raw<AuthConfigResponse | null>(null);
  let configError = $state<string | null>(null);
  let checking = $state(true);

  const features = [
    { icon: Megaphone, title: "Ticker", text: "Post news to the front page and set when it expires." },
    { icon: Users, title: "Committee pages", text: "Arrange the blocks on your committee\u2019s page." },
    { icon: SquareKanban, title: "Projects", text: "Add projects with pictures, links and video." },
  ];

  async function loadConfig() {
    try {
      config = await get<AuthConfigResponse>("/auth/config", { redirectOnUnauthenticated: false });
      configError = null;
    } catch (error) {
      configError = isApiError(error)
        ? error.message
        : "We could not load the sign-in options. Try again in a moment.";
    }
  }

  async function continueIfSignedIn(): Promise<boolean> {
    const me = await session.load();
    if (me === null) return false;
    await goto(returnTo, { replace: true });
    return true;
  }

  onMount(async () => {
    const [signedIn] = await Promise.all([continueIfSignedIn(), loadConfig()]);
    if (!signedIn) checking = false;
  });

  async function afterDevSignIn() {
    if (!(await continueIfSignedIn())) {
      throw new Error("Signed in, but the session did not stick. Check that cookies are allowed.");
    }
  }
</script>

<svelte:head>
  <title>Sign in - ScottyLabs Portal</title>
</svelte:head>

{#if checking}
  <CheckingScreen />
{:else}
  <div class="sign-in">
    <main class="sign-in__main" id="portal-main">
      <div class="sign-in__content">
        <p class="sign-in__brand">
          <DogMark width={34} />
          <span>ScottyLabs Portal</span>
        </p>

        <div class="sign-in__heading">
          <h1>One place to run scottylabs.org.</h1>
          <p class="sign-in__lede">
            Post to the ticker, keep your committee page current and show your projects.
          </p>
        </div>

        {#if notice}
          <Callout tone={notice.tone} role={notice.tone === "error" ? "alert" : "status"}>
            {notice.message}
          </Callout>
        {/if}

        <div class="sign-in__action">
          {#if config !== null && !config.oidc}
            <Callout tone="info" title="ScottyLabs sign-in is not set up for this site yet">
              Single sign-on starts working here once this site is connected to Keycloak.
              {#if config.devLogin}Until then, use development sign-in below.{/if}
            </Callout>
            <Button size="lg" icon={LogIn} disabled>Continue with ScottyLabs</Button>
          {:else}
            <Button size="lg" icon={LogIn} href={loginHref} data-sveltekit-reload class="sign-in__continue">
              Continue with ScottyLabs
            </Button>
          {/if}
          <p class="sign-in__idp">idp.scottylabs.org</p>
        </div>

        {#if configError}
          <Callout tone="warning" role="status">{configError}</Callout>
        {/if}

        <p class="sign-in__note">
          Single sign-on runs on Keycloak, the same login as the other ScottyLabs tools. No new
          password.
        </p>

        <hr class="sign-in__divider" />

        <p class="sign-in__help">
          Editing is for committee leads and directors. Not sure you have access? Ask the director of
          your committee.
        </p>

        {#if config?.devLogin}
          <DevSignIn
            personas={config.devPersonas}
            needsKey={config.devLoginNeedsKey}
            onsignedin={afterDevSignIn}
          />
        {/if}
      </div>
    </main>

    <aside class="sign-in__panel" aria-label="What you can manage">
      <img class="sign-in__art" src={heroArt} alt="" />
      <ul class="sign-in__features">
        {#each features as feature (feature.title)}
          <li class="sign-in__feature">
            <span class="sign-in__feature-icon">
              <feature.icon size={18} strokeWidth={1.75} aria-hidden="true" />
            </span>
            <span>
              <span class="sign-in__feature-title">{feature.title}</span>
              <span class="sign-in__feature-text">{feature.text}</span>
            </span>
          </li>
        {/each}
      </ul>
    </aside>
  </div>
{/if}

<style>
  .sign-in {
    display: grid;
    grid-template-columns: 1fr 1fr;
    min-height: 100vh;
    min-height: 100dvh;
    background: var(--p-surface);
    color: var(--p-text);
    font-family: var(--p-font);
    font-weight: 400;
  }
  .sign-in__main {
    display: flex;
    align-items: center;
    padding: 64px 40px 64px 120px;
    &:focus {
      outline: none;
    }
  }
  .sign-in__content {
    display: flex;
    flex-direction: column;
    gap: 28px;
    width: 100%;
    max-width: 460px;
  }
  .sign-in__brand {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
  }
  .sign-in__heading {
    display: flex;
    flex-direction: column;
    gap: 14px;

    & h1 {
      margin: 0;
      font-size: 2.75rem;
      font-weight: 700;
      line-height: 50px;
      letter-spacing: -0.02em;
    }
  }
  .sign-in__lede {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1.125rem;
    line-height: 28px;
  }
  .sign-in__action {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .sign-in__idp {
    margin: 0;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .sign-in__note {
    margin: 0;
    color: var(--p-text-tertiary);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .sign-in__divider {
    width: 100%;
    height: 1px;
    margin: 0;
    border: 0;
    background: var(--p-border);
  }
  .sign-in__help {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 0.875rem;
    line-height: 20px;
  }

  .sign-in__panel {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    padding: 0 56px 78px;
    overflow: hidden;
    background: var(--p-surface-muted);
  }
  .sign-in__art {
    position: absolute;
    top: 70px;
    left: 40px;
    width: 1284px;
    max-width: none;
    height: auto;
    pointer-events: none;
    user-select: none;
  }
  .sign-in__features {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 440px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sign-in__feature {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px 14px 16px;
    border: 1px solid var(--p-border);
    border-radius: var(--p-radius-lg);
    background: var(--p-surface);
    box-shadow: var(--p-shadow-card);
  }
  .sign-in__feature-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: var(--p-radius);
    background: var(--p-surface-muted);
  }
  .sign-in__feature-title {
    display: block;
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 17px;
  }
  .sign-in__feature-text {
    display: block;
    color: var(--p-text-secondary);
    font-size: 0.8125rem;
    line-height: 17px;
  }

  @media (max-width: 1100px) {
    .sign-in__main {
      padding-left: 56px;
    }
  }
  @media (max-width: 899px) {
    .sign-in {
      grid-template-columns: 1fr;
      grid-template-rows: auto auto;
    }
    .sign-in__main {
      align-items: flex-start;
      padding: 64px 20px 40px;
    }
    .sign-in__content {
      max-width: 560px;
      gap: 24px;
    }
    .sign-in__heading h1 {
      font-size: 2.5rem;
      line-height: 46px;
    }
    .sign-in__action :global(.sign-in__continue) {
      width: 100%;
    }
    .sign-in__panel {
      min-height: 300px;
      padding: 0;
      background: var(--p-surface);
    }
    .sign-in__art {
      top: 0;
      left: -40px;
      width: 760px;
    }
    .sign-in__features {
      display: none;
    }
  }
</style>
