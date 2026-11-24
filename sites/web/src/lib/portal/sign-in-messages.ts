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
