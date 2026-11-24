// The signed-in person for this portal visit, loaded once from GET /api/me and shared through
// Svelte context. The portal layout creates it; pages read it with getPortalSession().

import { createContext } from "svelte";
import { goto } from "$app/navigation";
import { ApiError, get, isApiError, post, setCsrfToken } from "../api/client.ts";
import type { MeResponse, Permissions, RoleAssignment } from "../api/types.ts";
import type { AccessState, SessionStatus, SignedOutReason } from "./access.ts";
import { replaceLocation } from "./navigation.ts";
import { SIGN_IN_PATH, isBarePortalPath } from "./paths.ts";

export type { SessionStatus, SignedOutReason } from "./access.ts";

export class PortalSession implements AccessState {
  /** "ready" once /api/me answered; "signed_out" on 401; "error" when it could not be reached. */
  status = $state<SessionStatus>("idle");
  me = $state.raw<MeResponse | null>(null);
  /** Why the last load failed, when status is "error". */
  error = $state.raw<ApiError | null>(null);
  /** Set when status is "signed_out". */
  signedOutReason = $state<SignedOutReason | null>(null);

  #inFlight: Promise<MeResponse | null> | null = null;

  /** The signed-in person. Only call inside the portal shell, which renders once this is ready. */
  get current(): MeResponse {
    if (this.me === null) throw new Error("The portal session is not loaded yet.");
    return this.me;
  }

  get user(): MeResponse["user"] | null {
    return this.me?.user ?? null;
  }

  get roles(): readonly RoleAssignment[] {
    return this.me?.roles ?? [];
  }

  get permissions(): Permissions | null {
    return this.me?.permissions ?? null;
  }

  get csrfToken(): string | null {
    return this.me?.csrfToken ?? null;
  }

  /** True when the person has at least one role. */
  get canUsePortal(): boolean {
    return this.me?.permissions.canUsePortal ?? false;
  }

  /** Loads /api/me unless it already has. Concurrent calls share one request. */
  load(): Promise<MeResponse | null> {
    if (this.status === "ready") return Promise.resolve(this.me);
    return this.#fetch(false);
  }

  /** Fetches /api/me again, for example after roles change. Keeps showing the current data. */
  refresh(): Promise<MeResponse | null> {
    return this.#fetch(true);
  }

  /** Ends the session on the server, forgets it here and goes to the sign-in page. */
  async signOut(): Promise<void> {
    try {
      await post("/auth/logout", undefined, { redirectOnUnauthenticated: false });
    } catch (error) {
      // Already signed out is fine. Anything else is worth telling the person about.
      if (!isApiError(error) || error.status !== 401) throw error;
    }
    this.markSignedOut("signed_out");
    await replaceLocation(
      `${SIGN_IN_PATH}?signedOut=1`,
      () => !isBarePortalPath(location.pathname),
      (href) => goto(href, { replace: true }),
    );
  }

  /** Forgets the session locally, for example after a 401. */
  markSignedOut(reason: SignedOutReason): void {
    this.me = null;
    this.error = null;
    this.signedOutReason = reason;
    this.status = "signed_out";
    setCsrfToken(null);
  }

  #fetch(keepCurrent: boolean): Promise<MeResponse | null> {
    if (this.#inFlight !== null) return this.#inFlight;
    if (!keepCurrent || this.me === null) this.status = "loading";
    const request = this.#request().finally(() => {
      this.#inFlight = null;
    });
    this.#inFlight = request;
    return request;
  }

  async #request(): Promise<MeResponse | null> {
    try {
      const me = await get<MeResponse>("/me", { redirectOnUnauthenticated: false });
      this.me = me;
      this.error = null;
      this.signedOutReason = null;
      this.status = "ready";
      setCsrfToken(me.csrfToken);
      return me;
    } catch (error) {
      if (isApiError(error) && error.status === 401) {
        this.markSignedOut(this.me === null ? "not_signed_in" : "expired");
        return null;
      }
      this.error = isApiError(error)
        ? error
        : new ApiError(0, "unexpected_response", "We could not check your access. Try again.");
      // A failed refresh keeps the data already on screen.
      if (this.me === null) this.status = "error";
      return this.me;
    }
  }
}

const [getSession, setSession] = createContext<PortalSession>();

/** The portal session. Available in every component inside the portal layout. */
export function getPortalSession(): PortalSession {
  return getSession();
}

export function setPortalSession(session: PortalSession): PortalSession {
  return setSession(session);
}
