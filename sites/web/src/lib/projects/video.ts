// Videos on project pages play from YouTube or Vimeo, loaded only after the visitor presses play,
// from the privacy-enhanced players (youtube-nocookie.com and Vimeo with do-not-track).

export interface VideoEmbed {
  provider: "youtube" | "vimeo";
  /** The player address for the iframe, which starts playing when loaded. */
  embedUrl: string;
  /** "YouTube" or "Vimeo", for labels. */
  providerName: string;
}

const YOUTUBE_ID = /^[A-Za-z0-9_-]{11}$/u;
const VIMEO_ID = /^\d{1,12}$/u;
const VIMEO_HASH = /^[0-9a-f]{6,20}$/iu;

/** The embed for a YouTube or Vimeo link, or null for anything else. */
export function videoEmbed(url: string): VideoEmbed | null {
  let parsed: URL;
  try {
    parsed = new URL(url.trim());
  } catch {
    return null;
  }
  if (parsed.protocol !== "https:") return null;
  const host = parsed.hostname.replace(/^(www\.|m\.)/u, "");
  if (host === "youtube.com" || host === "youtu.be" || host === "youtube-nocookie.com") {
    return youtube(parsed, host);
  }
  if (host === "vimeo.com" || host === "player.vimeo.com") return vimeo(parsed);
  return null;
}

function youtube(url: URL, host: string): VideoEmbed | null {
  const segments = url.pathname.split("/").filter((segment) => segment !== "");
  let id: string | undefined;
  if (host === "youtu.be") id = segments[0];
  else if (segments[0] === "watch") id = url.searchParams.get("v") ?? undefined;
  else if (["embed", "shorts", "live", "v"].includes(segments[0] ?? "")) id = segments[1];
  if (id === undefined || !YOUTUBE_ID.test(id)) return null;
  const params = new URLSearchParams({ autoplay: "1", rel: "0" });
  const start = startSeconds(url.searchParams.get("t") ?? url.searchParams.get("start"));
  if (start !== null) params.set("start", start.toString());
  return {
    provider: "youtube",
    providerName: "YouTube",
    embedUrl: `https://www.youtube-nocookie.com/embed/${id}?${params.toString()}`,
  };
}

function vimeo(url: URL): VideoEmbed | null {
  const segments = url.pathname.split("/").filter((segment) => segment !== "");
  const [first, second] = url.hostname === "player.vimeo.com" ? segments.slice(1) : segments;
  if (first === undefined || !VIMEO_ID.test(first)) return null;
  const params = new URLSearchParams({ autoplay: "1", dnt: "1" });
  const hash = url.searchParams.get("h") ?? second;
  if (hash !== undefined && VIMEO_HASH.test(hash)) params.set("h", hash);
  return {
    provider: "vimeo",
    providerName: "Vimeo",
    embedUrl: `https://player.vimeo.com/video/${first}?${params.toString()}`,
  };
}

/** Seconds from "90", "90s" or "1m30s". */
function startSeconds(value: string | null): number | null {
  if (value === null) return null;
  const match = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s?)?$/u.exec(value);
  if (match === null) return null;
  const [, hours = "0", minutes = "0", seconds = "0"] = match;
  const total = Number(hours) * 3600 + Number(minutes) * 60 + Number(seconds);
  return total > 0 ? total : null;
}
