// Turns a YouTube or Vimeo link into a privacy-enhanced embed address. Nothing loads from those sites until a visitor chooses Play.

export interface VideoEmbed {
  provider: "YouTube" | "Vimeo";
  src: string;
}

const YOUTUBE_ID = /^[\w-]{6,20}$/u;
const VIMEO_ID = /^\d{3,12}$/u;

function parse(url: string): URL | null {
  try {
    const parsed = new URL(url.trim());
    return parsed.protocol === "https:" ? parsed : null;
  } catch {
    return null;
  }
}

function youtubeId(url: URL, host: string): string | null {
  if (host === "youtu.be") return url.pathname.split("/")[1] ?? null;
  if (url.pathname === "/watch") return url.searchParams.get("v");
  const [, kind, id] = url.pathname.split("/");
  return kind === "embed" || kind === "shorts" || kind === "live" ? (id ?? null) : null;
}

function vimeoId(url: URL, host: string): string | null {
  const parts = url.pathname.split("/").filter((part) => part !== "");
  if (host === "player.vimeo.com") return parts[0] === "video" ? (parts[1] ?? null) : null;
  return parts.find((part) => VIMEO_ID.test(part)) ?? null;
}

/** The embed for a YouTube or Vimeo watch link, or null for anything else. */
export function videoEmbed(link: string): VideoEmbed | null {
  const url = parse(link);
  if (url === null) return null;
  const host = url.hostname.replace(/^(www\.|m\.)/u, "");
  if (host === "youtube.com" || host === "youtu.be") {
    const id = youtubeId(url, host);
    if (id === null || !YOUTUBE_ID.test(id)) return null;
    return {
      provider: "YouTube",
      src: `https://www.youtube-nocookie.com/embed/${id}?autoplay=1&rel=0`,
    };
  }
  if (host === "vimeo.com" || host === "player.vimeo.com") {
    const id = vimeoId(url, host);
    if (id === null || !VIMEO_ID.test(id)) return null;
    return { provider: "Vimeo", src: `https://player.vimeo.com/video/${id}?autoplay=1&dnt=1` };
  }
  return null;
}
