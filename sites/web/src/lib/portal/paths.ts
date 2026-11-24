// Portal paths and the sign-in return address.

export const PORTAL_HOME = "/portal/";
export const SIGN_IN_PATH = "/portal/sign-in/";
export const NO_ACCESS_PATH = "/portal/no-access/";

/** Pages that render without the portal shell or the access check. */
const BARE_PATHS = [SIGN_IN_PATH];

export function isBarePortalPath(pathname: string): boolean {
  return BARE_PATHS.some((path) => pathname === path || pathname === path.slice(0, -1));
}

/**
 * A safe place to return to after signing in: a portal path on this origin, never the sign-in
 * page itself. Anything else becomes the portal home, so the parameter cannot redirect off site.
 */
export function safeReturnTo(value: string | null | undefined): string {
  if (typeof value !== "string" || value.length > 2048) return PORTAL_HOME;
  // Reject protocol-relative and backslash tricks before parsing.
  if (!value.startsWith("/portal") || value.startsWith("//") || /[\\\s]/u.test(value)) {
    return PORTAL_HOME;
  }
  let url: URL;
  try {
    url = new URL(value, "https://scottylabs.invalid");
  } catch {
    return PORTAL_HOME;
  }
  if (url.origin !== "https://scottylabs.invalid") return PORTAL_HOME;
  if (url.pathname !== "/portal" && !url.pathname.startsWith("/portal/")) return PORTAL_HOME;
  if (isBarePortalPath(url.pathname)) return PORTAL_HOME;
  return `${url.pathname}${url.search}${url.hash}`;
}

/** The sign-in page, coming back to `returnTo` afterwards. */
export function signInHref(returnTo: string, extra: Record<string, string> = {}): string {
  const params = new URLSearchParams({ returnTo: safeReturnTo(returnTo), ...extra });
  return `${SIGN_IN_PATH}?${params.toString()}`;
}
