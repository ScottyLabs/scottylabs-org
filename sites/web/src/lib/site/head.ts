// The site service writes a project's title, description and Open Graph tags into the page shell (services/site/src/static_files.rs), for crawlers and link previews that do not run scripts.
// Each one carries INJECTED_ATTRIBUTE. Once the app runs, every page writes its own head, so those tags go: otherwise a project page would have two descriptions, and after moving to another page the head would still describe the first project.

export const INJECTED_ATTRIBUTE = "data-sl-injected";

interface HeadTags {
  querySelectorAll(selectors: string): Iterable<{ remove(): void }>;
}

/** Removes the tags the service injected into `head`. Call it before any page writes its head. */
export function removeInjectedHeadTags(head: HeadTags): void {
  for (const tag of head.querySelectorAll(`[${INJECTED_ATTRIBUTE}]`)) tag.remove();
}
