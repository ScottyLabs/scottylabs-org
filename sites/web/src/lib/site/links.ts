// Short labels for links, shared by the ticker, project cards and pages, and approvals.

/**
 * The host of a link for a short label: "tartanhacks.com" for https://www.tartanhacks.com/x.
 * Null when it is not a web address, or, with `httpsOnly`, not an https one.
 */
export function linkHost(
  url: string | null | undefined,
  { httpsOnly = false }: { httpsOnly?: boolean } = {},
): string | null {
  if (url === null || url === undefined || url.trim() === "") return null;
  let parsed: URL;
  try {
    parsed = new URL(url.trim());
  } catch {
    return null;
  }
  if (parsed.protocol !== "https:" && (httpsOnly || parsed.protocol !== "http:")) return null;
  return parsed.hostname.replace(/^www\./u, "");
}
