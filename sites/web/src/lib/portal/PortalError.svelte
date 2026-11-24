<script lang="ts">
  // The portal's error card. Inside the shell it fills the content area; `standalone` adds a plain
  // brand bar for addresses that never reached the portal layout.
  import CircleAlert from "@lucide/svelte/icons/circle-alert";
  import Lock from "@lucide/svelte/icons/lock";
  import MapPinOff from "@lucide/svelte/icons/map-pin-off";
  import { Button, Card, DogMark } from "./ui/index.ts";
  import { PORTAL_HOME } from "./paths.ts";

  let {
    status,
    message,
    standalone = false,
  }: { status: number; message?: string; standalone?: boolean } = $props();

  // SvelteKit's own wording ("Not Found", "Internal Error") is not written for people.
  const GENERIC = new Set(["Not Found", "Internal Error", "Error"]);

  const copy = $derived.by(() => {
    const custom = message !== undefined && !GENERIC.has(message) ? message : null;
    if (status === 404) {
      return {
        icon: MapPinOff,
        title: "We could not find that page",
        body:
          custom ??
          "The address may be mistyped, or the page may have moved. The overview links to everything you can edit.",
        retry: false,
      };
    }
    if (status === 403) {
      return {
        icon: Lock,
        title: "This page is not open to you",
        body: custom ?? "Ask the director of your committee if you think you should have access.",
        retry: false,
      };
    }
    return {
      icon: CircleAlert,
      title: "Something went wrong",
      body:
        custom ??
        "Try again in a moment. If it keeps happening, let the Tech committee know what you were doing.",
      retry: true,
    };
  });
</script>

<svelte:head>
  <title>{copy.title} - ScottyLabs Portal</title>
</svelte:head>

<div class={["portal-error", standalone && "portal-error--standalone"]}>
  {#if standalone}
    <header class="portal-error__bar">
      <a class="portal-error__brand" href={PORTAL_HOME}>
        <DogMark width={27} />
        <span>Portal</span>
      </a>
    </header>
  {/if}
  <svelte:element
    this={standalone ? "main" : "div"}
    class="portal-error__main"
    id={standalone ? "portal-main" : undefined}
  >
    <Card padding="lg" class="portal-error__card">
      <span class="portal-error__icon"><copy.icon size={20} strokeWidth={1.75} aria-hidden="true" /></span>
      <p class="portal-error__status">Error {status}</p>
      <h1 class="portal-error__title" tabindex="-1">{copy.title}</h1>
      <p class="portal-error__body">{copy.body}</p>
      <div class="portal-error__actions">
        <Button href={PORTAL_HOME}>Go to the overview</Button>
        {#if copy.retry}
          <Button variant="outline" onclick={() => location.reload()}>Try again</Button>
        {/if}
      </div>
    </Card>
  </svelte:element>
</div>

<style>
  .portal-error {
    color: var(--p-text);
    font-family: var(--p-font);
  }
  .portal-error--standalone {
    min-height: 100vh;
    background: var(--p-bg);
  }
  .portal-error__bar {
    display: flex;
    align-items: center;
    height: var(--p-topbar-height);
    padding: 0 40px;
    border-bottom: 1px solid var(--p-border);
    background: var(--p-surface);
    @media (max-width: 899px) {
      height: var(--p-topbar-height-mobile);
      padding: 0 16px;
    }
  }
  .portal-error__brand {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    color: var(--p-text);
    font-size: 1rem;
    font-weight: 500;
    border-radius: 6px;
    &:focus-visible {
      outline: 2px solid var(--p-focus);
      outline-offset: 4px;
    }
  }
  .portal-error__main {
    display: flex;
    justify-content: center;
    padding: 48px 0;
    @media (max-width: 600px) {
      padding-top: 24px;
    }
  }
  .portal-error--standalone .portal-error__main {
    padding: 96px 16px 48px;
    @media (max-width: 600px) {
      padding-top: 40px;
    }
  }
  .portal-error__main :global(.portal-error__card) {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: 100%;
    max-width: 520px;
  }
  .portal-error__icon {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    background: var(--p-surface-muted);
  }
  .portal-error__status {
    margin: 4px 0 -8px;
    color: var(--p-text-tertiary);
    font-family: var(--p-mono);
    font-size: 0.75rem;
    line-height: 16px;
  }
  .portal-error__title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 500;
    line-height: 28px;
    &:focus {
      outline: none;
    }
  }
  .portal-error__body {
    margin: 0;
    color: var(--p-text-secondary);
    font-size: 1rem;
    line-height: 24px;
  }
  .portal-error__actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding-top: 6px;
  }
</style>
