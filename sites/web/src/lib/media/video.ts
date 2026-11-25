// Turns a YouTube or Vimeo link into the privacy-enhanced player's address (youtube-nocookie.com,
// and Vimeo with do-not-track). Nothing loads from those sites until a visitor chooses Play.
// Committee pages and project pages read links the same way. Which links play mirrors `blocks::video_id` in services/site/src/blocks.rs; keep the two in step.

export interface VideoEmbed {
  provider: "youtube" | "vimeo";
  /** "YouTube" or "Vimeo", for labels. */
  providerName: "YouTube" | "Vimeo";
  /** The player address for the iframe, which starts playing when loaded. */
  embedUrl: string;
}

const YOUTUBE_ID = /^[\w-]{6,20}$/u;
const VIMEO_ID = /^\d{3,12}$/u;
/** The key of an unlisted Vimeo video. */
const VIMEO_HASH = /^[0-9a-f]{6,20}$/iu;

function parse(link: string): URL | null {
  try {
    const url = new URL(link.trim());
    return url.protocol === "https:" ? url : null;
  } catch {
    return null;
  }
}

/** The embed for a link to one YouTube or Vimeo video, or null for anything else. */
export function videoEmbed(link: string): VideoEmbed | null {
  const url = parse(link);
  if (url === null) return null;
  const host = url.hostname.replace(/^(www\.|m\.)/u, "");
  if (host === "youtube.com" || host === "youtu.be") return youtube(url, host);
  if (host === "vimeo.com" || host === "player.vimeo.com") return vimeo(url, host);
  return null;
}

/** Watch, youtu.be, embed, shorts and live links. */
function youtube(url: URL, host: string): VideoEmbed | null {
  const parts = url.pathname.split("/");
  let id: string | null;
  if (host === "youtu.be") id = parts[1] ?? null;
  else if (url.pathname === "/watch") id = url.searchParams.get("v");
  else id = ["embed", "shorts", "live"].includes(parts[1] ?? "") ? (parts[2] ?? null) : null;
  if (id === null || !YOUTUBE_ID.test(id)) return null;
  const params = new URLSearchParams({ autoplay: "1", rel: "0" });
  const start = startSeconds(url.searchParams.get("t") ?? url.searchParams.get("start"));
  if (start !== null) params.set("start", start.toString());
  return {
    provider: "youtube",
    providerName: "YouTube",
    embedUrl: `https://www.youtube-nocookie.com/embed/${id}?${params.toString()}`,
  };
}

/** Links with a video number: vimeo.com/<number>, also inside channels, and the player's own. */
function vimeo(url: URL, host: string): VideoEmbed | null {
  const parts = url.pathname.split("/").filter((part) => part !== "");
  let index: number;
  if (host === "player.vimeo.com")
    index = parts[0] === "video" && VIMEO_ID.test(parts[1] ?? "") ? 1 : -1;
  else index = parts.findIndex((part) => VIMEO_ID.test(part));
  const id = parts[index];
  if (index < 0 || id === undefined) return null;
  const params = new URLSearchParams({ autoplay: "1", dnt: "1" });
  const hash = url.searchParams.get("h") ?? parts[index + 1];
  if (hash !== undefined && VIMEO_HASH.test(hash)) params.set("h", hash);
  return {
    provider: "vimeo",
    providerName: "Vimeo",
    embedUrl: `https://player.vimeo.com/video/${id}?${params.toString()}`,
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
