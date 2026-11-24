// Where the portal sends someone, decided from their session alone. Kept free of SvelteKit imports so the rules can be tested without a browser.

import {
  NO_ACCESS_PATH,
  PORTAL_HOME,
  isBarePortalPath,
  safeReturnTo,
  signInHref,
} from "./paths.ts";

export type SessionStatus = "idle" | "loading" | "ready" | "signed_out" | "error";

/**
 * Why the session is signed out: the person chose to sign out, the session expired while they
 * were using the portal, or they were never signed in.
 */
export type SignedOutReason = "signed_out" | "expired" | "not_signed_in";

/** The parts of the portal session that decide where someone may be. */
export interface AccessState {
  readonly status: SessionStatus;
  readonly signedOutReason: SignedOutReason | null;
  readonly canUsePortal: boolean;
}

export function isNoAccessPath(pathname: string): boolean {
  return pathname === NO_ACCESS_PATH || pathname === NO_ACCESS_PATH.slice(0, -1);
}

/**
 * Where the portal shell must send someone who is on `url`, or null when they may stay. Signed out
 * people go to sign-in and come back here afterwards; people without a role go to the no-access
 * page; people with a role never see it. Choosing to sign out navigates on its own, so it is left
 * alone here.
 */
export function portalRedirect(
  url: { readonly pathname: string; readonly search: string },
  state: AccessState,
): string | null {
  if (isBarePortalPath(url.pathname)) return null;
  if (state.status === "signed_out") {
    if (state.signedOutReason === "signed_out") return null;
    const extra = state.signedOutReason === "expired" ? { reason: "session_ended" } : undefined;
    return signInHref(`${url.pathname}${url.search}`, extra);
  }
  if (state.status !== "ready") return null;
  const onNoAccessPage = isNoAccessPath(url.pathname);
  if (state.canUsePortal && onNoAccessPage) return PORTAL_HOME;
  if (!state.canUsePortal && !onNoAccessPage) return NO_ACCESS_PATH;
  return null;
}

/**
 * Where to go once someone is signed in. Deciding before the first navigation means people
 * without a role go straight to the no-access page instead of through a second redirect.
 */
export function destinationAfterSignIn(canUsePortal: boolean, returnTo: string): string {
  return canUsePortal ? safeReturnTo(returnTo) : NO_ACCESS_PATH;
}
