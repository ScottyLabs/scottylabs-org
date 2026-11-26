// Where the browser goes after signing out. Plain logic, so it can be tested without a browser.

/** True for this machine, where Keycloak may run over plain http in development. */
function isLoopback(hostname: string): boolean {
  return hostname === "localhost" || hostname === "127.0.0.1" || hostname === "[::1]";
}

/**
 * The Keycloak address that ends the single sign-on session as well, when POST /api/auth/logout
 * answers with one as `{ "endSessionUrl": "..." }`. Only an absolute https address passes (http
 * only on this machine); anything else, including the usual empty 204, gives null and the person
 * goes to the portal's sign-in page instead.
 */
export function endSessionUrl(body: unknown): string | null {
  if (typeof body !== "object" || body === null || !("endSessionUrl" in body)) return null;
  const value = body.endSessionUrl;
  if (typeof value !== "string") return null;
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  if (url.username !== "" || url.password !== "") return null;
  if (url.protocol === "https:" || (url.protocol === "http:" && isLoopback(url.hostname))) {
    return url.href;
  }
  return null;
}
