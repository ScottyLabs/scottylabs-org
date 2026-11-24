// Turns what someone types into the link field into a link the server accepts.

import { safeHref } from "./sanitize.ts";

export type LinkInputResult = { ok: true; href: string } | { ok: false; message: string };

const EMAIL = /^[^\s@/:]+@[^\s@/:]+\.[^\s@/:]+$/u;

/**
 * Accepts `https://` and `mailto:` links. A bare address such as `scottylabs.org` gets
 * `https://`, and a bare email address gets `mailto:`.
 */
export function linkFromInput(input: string): LinkInputResult {
  const value = input.trim();
  if (value === "") return { ok: false, message: "Enter a link that starts with https://." };
  if (/^http:\/\//iu.test(value)) {
    return { ok: false, message: "Use a secure link that starts with https://." };
  }
  let candidate = value;
  if (EMAIL.test(value)) {
    candidate = `mailto:${value}`;
  } else if (/^[a-z][a-z0-9+.-]*:/iu.test(value)) {
    if (!/^(https:\/\/|mailto:)/iu.test(value)) {
      return { ok: false, message: "Links must start with https:// or mailto:." };
    }
  } else {
    candidate = `https://${value}`;
  }
  const href = safeHref(candidate);
  if (href === null || (href.startsWith("https:") && !isPlausibleHost(href))) {
    return { ok: false, message: "That does not look like a link. Check it and try again." };
  }
  return { ok: true, href };
}

function isPlausibleHost(href: string): boolean {
  const { hostname } = new URL(href);
  return hostname.includes(".") && !hostname.startsWith(".") && !hostname.endsWith(".");
}

/** What a link shows when it is inserted without selected text. */
export function linkText(href: string): string {
  const text = href.startsWith("mailto:")
    ? href.slice("mailto:".length)
    : href.replace(/^https:\/\//u, "").replace(/\/$/u, "");
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}
