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

/** Keycloak's end-session request as a form to post: where it goes, and its fields. */
export interface EndSessionForm {
  action: string;
  fields: [name: string, value: string][];
}

/**
 * The end-session address of `endSessionUrl` as a form to post instead of an address to open:
 * its query (the ID token as `id_token_hint`, and `client_id`) becomes hidden fields, so the ID
 * token, which names the person, never lands in the browser's history, in access logs or in a
 * Referer. Keycloak takes the same fields posted as a form (OpenID RP-Initiated Logout).
 */
export function endSessionForm(body: unknown): EndSessionForm | null {
  const href = endSessionUrl(body);
  if (href === null) return null;
  const url = new URL(href);
  const fields = [...url.searchParams.entries()];
  url.search = "";
  url.hash = "";
  return { action: url.href, fields };
}
