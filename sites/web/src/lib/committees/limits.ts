// Text limits for committee pages, in characters after trimming. Mirrors `blocks::limits` in services/site/src/blocks.rs, which enforces them when a draft is saved.

export const LIMITS = {
  tagline: 120,
  heading: 80,
  caption: 200,
  /** Quotes, the call to action text and the teams introduction. */
  shortText: 240,
  statValue: 12,
  statLabel: 60,
  teamName: 40,
  teamAbout: 140,
  attribution: 80,
  buttonLabel: 40,
  videoTitle: 80,
  indexDescription: 300,
  teamsLine: 120,
  meetingTime: 60,
  meetingRoom: 80,
  channel: 80,
  maxTeams: 20,
  minStats: 2,
  maxStats: 4,
  minGallery: 3,
  maxGallery: 6,
  /** Visible characters of a text block's body; the same limit as project descriptions. */
  body: 4000,
} as const;

/** Keeps text on one line, as the service requires for every plain text field. */
export function oneLine(text: string): string {
  return text.replaceAll(/[\r\n]+/gu, " ");
}

/** A Slack channel as typed, without the leading # and in lower case, as the service stores it. */
export function cleanChannel(text: string): string {
  return text.trim().replace(/^#+/u, "").toLowerCase();
}
