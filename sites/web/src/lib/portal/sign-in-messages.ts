// What the sign-in page says after a redirect back to it (?error=, ?signedOut=, ?reason=).

export interface SignInNotice {
  tone: "info" | "success" | "warning" | "error";
  message: string;
}

const ERRORS: Record<string, SignInNotice> = {
  cancelled: {
    tone: "warning",
    message: "Sign-in was cancelled. Choose Continue with ScottyLabs to try again.",
  },
  expired: {
    tone: "warning",
    message: "Sign-in took too long and timed out. Try again.",
  },
  failed: {
    tone: "error",
    message:
      "We could not sign you in. Try again, and if it keeps happening, let the Tech committee know.",
  },
  not_configured: {
    tone: "info",
    message: "ScottyLabs sign-in is not set up for this site yet.",
  },
  unavailable: {
    tone: "warning",
    message:
      "ScottyLabs sign-in (Keycloak) is not responding right now. Try again in a few minutes.",
  },
  rate_limited: {
    tone: "warning",
    message: "There were too many sign-in attempts from your network. Wait a minute and try again.",
  },
  no_andrew_id: {
    tone: "error",
    message:
      "We couldn\u2019t match this account to a CMU Andrew ID. Sign in with your Andrew account, or ask an admin for help.",
  },
  linked_elsewhere: {
    tone: "error",
    message:
      "Your Andrew ID is linked to a different ScottyLabs account. Ask an admin to reset your sign-in, then try again.",
  },
};

export function signInNotice(params: Pick<URLSearchParams, "get" | "has">): SignInNotice | null {
  const error = params.get("error");
  if (error !== null) return ERRORS[error] ?? ERRORS.failed ?? null;
  if (params.has("signedOut")) return { tone: "success", message: "You are signed out." };
  if (params.get("reason") === "session_ended") {
    return {
      tone: "info",
      message: "Your session ended. Sign in again to pick up where you left off.",
    };
  }
  return null;
}
