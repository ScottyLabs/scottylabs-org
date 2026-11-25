// Which failed draft saves are worth repeating without being asked.

import { isApiError } from "../../api/client.ts";

/**
 * True when the connection dropped, the server was busy (429) or failed (5xx): trying again later
 * can work. Losing edit access (403), a project that is gone (404), a conflict (409) or a draft
 * that is too large (413) stays the same until something changes, so those stop with the
 * server's sentence. Sign-in problems (401) are the portal's to handle.
 */
export function retriesOnItsOwn(error: unknown): boolean {
  if (!isApiError(error)) return false;
  return error.status === 0 || error.status === 429 || error.status >= 500;
}
