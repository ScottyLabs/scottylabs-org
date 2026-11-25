// Ticker helpers that need no DOM: expiry presets and the sentences around them.
// Dates are read and written in Pittsburgh time, like the rest of the portal.

import type { Announcement, TickerSource } from "../api/types.ts";
import {
  formatDay,
  formatRelative,
  formatTime,
  fromZonedInputValue,
  toZonedInputValue,
} from "../portal/dates.ts";

/** At most this many announcements are live at once. */
export const MAX_LIVE = 6;
/** Messages are one line of at most this many characters. */
export const MESSAGE_MAX = 90;
/** An announcement leaves the ticker at most this many days after it goes live. */
export const MAX_DAYS = 30;
/** Announcements leaving within this many days are flagged. */
export const SOON_DAYS = 3;

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Sources in the order the composer shows them: the committees, then ScottyLabs as a whole. */
export function sourceOrder(sources: readonly TickerSource[]): TickerSource[] {
  return [
    ...sources.filter((source) => source !== "club"),
    ...sources.filter((source) => source === "club"),
  ];
}

export type ExpiryPreset = "tomorrow" | "3d" | "1w" | "2w";

export const EXPIRY_PRESETS: readonly { value: ExpiryPreset; label: string; days: number }[] = [
  { value: "tomorrow", label: "Tomorrow", days: 1 },
  { value: "3d", label: "In 3 days", days: 3 },
  { value: "1w", label: "In 1 week", days: 7 },
  { value: "2w", label: "In 2 weeks", days: 14 },
];

/** "2026-11-24": the calendar day in Pittsburgh at this instant. */
export function zonedDay(value: Date | string): string {
  return toZonedInputValue(value).slice(0, 10);
}

/** The calendar day `days` after `day` ("2026-11-24" plus 7 is "2026-12-01"). */
export function addDays(day: string, days: number): string {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(Date.UTC(year, month - 1, date + days)).toISOString().slice(0, 10);
}

/** Whole calendar days from one Pittsburgh day to another. */
export function daysBetween(from: string, to: string): number {
  return Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / DAY);
}

/** An instant from a Pittsburgh day and a 24-hour "HH:MM" time, as an RFC 3339 string. */
export function zonedInstant(day: string, time: string): string | null {
  return fromZonedInputValue(`${day}T${time}`);
}

/** A preset's expiry: 11:59 PM Pittsburgh time, that many days after today. */
export function presetExpiry(preset: ExpiryPreset, now: Date = new Date()): string {
  const days = EXPIRY_PRESETS.find((option) => option.value === preset)?.days ?? 7;
  return (
    zonedInstant(addDays(zonedDay(now), days), "23:59") ??
    new Date(now.getTime() + days * DAY).toISOString()
  );
}

/** The preset that produces exactly this expiry, if one does. */
export function presetFor(expiresAt: string | null, now: Date = new Date()): ExpiryPreset | null {
  if (expiresAt === null) return null;
  return (
    EXPIRY_PRESETS.find(
      (option) => presetExpiry(option.value, now) === new Date(expiresAt).toISOString(),
    )?.value ?? null
  );
}

/** The latest moment an announcement that goes live now may stay. */
export function latestExpiry(now: Date = new Date()): Date {
  return new Date(now.getTime() + MAX_DAYS * DAY);
}

/** What is wrong with an expiry, in the words the service uses, or null when it is fine. */
export function expiryProblem(expiresAt: string | null, now: Date = new Date()): string | null {
  if (expiresAt === null) return "Pick when it should leave the ticker.";
  const at = Date.parse(expiresAt);
  if (Number.isNaN(at)) return "Pick when it should leave the ticker.";
  if (at <= now.getTime()) return "Pick a time in the future for it to leave the ticker.";
  if (at > latestExpiry(now).getTime()) {
    return `Pick a time within ${MAX_DAYS} days. Announcements stay on the ticker for ${MAX_DAYS} days at most.`;
  }
  return null;
}

/** "today", "tomorrow" or "7 days from now", counted in Pittsburgh calendar days. */
export function distanceInDays(expiresAt: string, now: Date = new Date()): string {
  const days = daysBetween(zonedDay(now), zonedDay(expiresAt));
  if (days <= 0) return "today";
  if (days === 1) return "tomorrow";
  return `${days} days from now`;
}

/** "Leaves the ticker on Tue, Dec 1 at 11:59 PM, 7 days from now." */
export function expirySentence(expiresAt: string, now: Date = new Date()): string {
  return `Leaves the ticker on ${formatDay(expiresAt, now)} at ${formatTime(expiresAt)}, ${distanceInDays(expiresAt, now)}.`;
}

/** "Tue, Dec 1, 9:00 AM", for the chosen date on the Pick a date button. */
export function shortDateTime(value: string, now: Date = new Date()): string {
  return `${formatDay(value, now)}, ${formatTime(value)}`;
}

export interface TimeLeft {
  /** "in 6 days". */
  label: string;
  /** How much of its time on the ticker is left, from 0 to 1. */
  fraction: number;
  /** Leaves within three days. */
  soon: boolean;
}

/** How long a live announcement has left, for the bar in the list. */
export function timeLeft(
  announcement: Pick<Announcement, "expiresAt" | "startsAt" | "createdAt">,
  now: Date = new Date(),
): TimeLeft {
  const end = Date.parse(announcement.expiresAt);
  const start = Date.parse(announcement.startsAt ?? announcement.createdAt);
  const remaining = end - now.getTime();
  const total = Math.max(end - start, MINUTE);
  return {
    label: formatRelative(announcement.expiresAt, now),
    fraction: Math.min(1, Math.max(0, remaining / total)),
    soon: remaining < SOON_DAYS * DAY,
  };
}

/** "Leaves in 6 days", or "Left 2 days ago" once it has gone. */
export function leavesLabel(expiresAt: string, now: Date = new Date()): string {
  const relative = formatRelative(expiresAt, now);
  return Date.parse(expiresAt) > now.getTime() ? `Leaves ${relative}` : `Left ${relative}`;
}

/** A message in curly quotes, for sentences about it. */
export function quoted(text: string): string {
  return "\u201C" + text + "\u201D";
}

/** "Thomas Kanz", "Thomas Kanz and Evan Foster", "A, B and C". */
export function listNames(names: readonly string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1) ?? ""}`;
}
