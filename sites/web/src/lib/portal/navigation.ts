// Moving someone to another page and making sure they arrive. Kept free of SvelteKit imports so it can be tested; callers pass the router's navigation in.

/** A client-side navigation that replaces the current history entry, such as SvelteKit's goto. */
export type Navigate = (href: string) => Promise<void>;

/**
 * Replaces the current page with `href`, through the router first. The router rejects paths it has
 * no route for, and a newer navigation can cut this one short without an error. In both cases,
 * when `stillWanted` says the person should still go, the browser loads `href` in full: they reach
 * the page (or the not-found page) instead of staying on a waiting screen.
 */
export async function replaceLocation(
  href: string,
  stillWanted: () => boolean,
  navigate: Navigate,
  load: (href: string) => void = loadInFull,
): Promise<void> {
  try {
    await navigate(href);
  } catch {
    // Not there yet; the check below decides whether to load the page in full.
  }
  if (stillWanted()) load(href);
}

function loadInFull(href: string): void {
  location.replace(href);
}
