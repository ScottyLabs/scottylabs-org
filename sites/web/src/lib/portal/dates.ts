// Dates and times as ScottyLabs reads them: in Pittsburgh time (America/New_York), whatever the viewer's own time zone. The API sends RFC 3339 strings in UTC.

export const SITE_TIME_ZONE = "America/New_York";

type DateInput = string | number | Date;

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(options: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const key = JSON.stringify(options);
  let cached = formatters.get(key);
  if (cached === undefined) {
    cached = new Intl.DateTimeFormat("en-US", { timeZone: SITE_TIME_ZONE, ...options });
    formatters.set(key, cached);
  }
  return cached;
}

function toDate(value: DateInput): Date | null {
  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

// Newer ICU puts a narrow no-break space before AM and PM. Plain spaces keep output predictable.
function clean(text: string): string {
  return text.replaceAll(/[\u202F\u00A0]/gu, " ");
}

function yearIn(date: Date): number {
  return Number(formatter({ year: "numeric" }).format(date));
}

function needsYear(date: Date, now: Date): boolean {
  return yearIn(date) !== yearIn(now);
}

/** "Tue, Dec 1, 9:00 AM". Adds the year when it is not this year. */
export function formatAbsolute(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  if (date === null) return "";
  const withYear = needsYear(date, toDate(now) ?? new Date());
  return clean(
    formatter({
      weekday: "short",
      month: "short",
      day: "numeric",
      ...(withYear ? { year: "numeric" } : {}),
      hour: "numeric",
      minute: "2-digit",
    }).format(date),
  );
}

/** "Fri, Dec 4 at 5:00 PM". */
export function formatDateTimeLong(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  if (date === null) return "";
  return `${formatDay(date, now)} at ${formatTime(date)}`;
}

/** "Tue, Nov 24". Adds the year when it is not this year. */
export function formatDay(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  if (date === null) return "";
  const withYear = needsYear(date, toDate(now) ?? new Date());
  return clean(
    formatter({
      weekday: "short",
      month: "short",
      day: "numeric",
      ...(withYear ? { year: "numeric" } : {}),
    }).format(date),
  );
}

/** "Nov 20", or "Nov 20, 2025" for another year. */
export function formatShortDate(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  if (date === null) return "";
  const withYear = needsYear(date, toDate(now) ?? new Date());
  return clean(
    formatter({ month: "short", day: "numeric", ...(withYear ? { year: "numeric" } : {}) }).format(
      date,
    ),
  );
}

/** "9:00 AM". */
export function formatTime(value: DateInput): string {
  const date = toDate(value);
  if (date === null) return "";
  return clean(formatter({ hour: "numeric", minute: "2-digit" }).format(date));
}

function plural(count: number, unit: string): string {
  return `${count} ${unit}${count === 1 ? "" : "s"}`;
}

/**
 * "in 6 days", "2 hours ago", "5 min ago", "just now", "tomorrow", "yesterday". Past a month it
 * counts months.
 */
export function formatRelative(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  const reference = toDate(now);
  if (date === null || reference === null) return "";
  const delta = date.getTime() - reference.getTime();
  const distance = Math.abs(delta);
  const future = delta > 0;
  const phrase = (amount: string) => (future ? `in ${amount}` : `${amount} ago`);

  if (distance < MINUTE) return "just now";
  if (distance < HOUR) return phrase(`${Math.round(distance / MINUTE)} min`);
  if (distance < DAY) return phrase(plural(Math.max(1, Math.round(distance / HOUR)), "hour"));
  const days = Math.round(distance / DAY);
  if (days === 1) return future ? "tomorrow" : "yesterday";
  if (days < 45) return phrase(plural(days, "day"));
  return phrase(plural(Math.round(days / 30), "month"));
}

/**
 * For "last updated" columns: "Just now" and "12 min ago" today, "Nov 20" otherwise.
 */
export function formatUpdated(value: DateInput, now: DateInput = new Date()): string {
  const date = toDate(value);
  const reference = toDate(now);
  if (date === null || reference === null) return "";
  const age = reference.getTime() - date.getTime();
  if (age >= 0 && age < MINUTE) return "Just now";
  if (age >= 0 && age < 12 * HOUR) return formatRelative(date, reference);
  return formatShortDate(date, reference);
}

/** "morning", "afternoon" or "evening" in Pittsburgh, for greetings. */
export function partOfDay(now: DateInput = new Date()): "morning" | "afternoon" | "evening" {
  const date = toDate(now) ?? new Date();
  const hour = Number(formatter({ hour: "numeric", hourCycle: "h23" }).format(date));
  if (hour < 12) return "morning";
  if (hour < 17) return "afternoon";
  return "evening";
}

interface WallTime {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
}

function wallTime(date: Date): WallTime {
  const parts = formatter({
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    Number(parts.find((p) => p.type === type)?.value ?? "0");
  return {
    year: part("year"),
    month: part("month"),
    day: part("day"),
    hour: part("hour"),
    minute: part("minute"),
  };
}

/** How far Pittsburgh's wall clock is ahead of UTC at this instant, in milliseconds. */
function zoneOffset(date: Date): number {
  const wall = wallTime(date);
  const asUtc = Date.UTC(wall.year, wall.month - 1, wall.day, wall.hour, wall.minute);
  return asUtc - Math.floor(date.getTime() / MINUTE) * MINUTE;
}

const pad = (value: number) => String(value).padStart(2, "0");

/**
 * The value for an `<input type="datetime-local">` showing this instant in Pittsburgh time:
 * "2026-12-01T09:00".
 */
export function toZonedInputValue(value: DateInput): string {
  const date = toDate(value);
  if (date === null) return "";
  const wall = wallTime(date);
  return `${wall.year}-${pad(wall.month)}-${pad(wall.day)}T${pad(wall.hour)}:${pad(wall.minute)}`;
}

/**
 * Reads an `<input type="datetime-local">` value as Pittsburgh time and returns the instant as
 * an RFC 3339 string in UTC, or null when the value is not a date and time.
 */
export function fromZonedInputValue(value: string): string | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/u.exec(value);
  if (match === null) return null;
  const [year, month, day, hour, minute] = match.slice(1).map(Number);
  const guess = Date.UTC(year, month - 1, day, hour, minute);
  // The offset depends on the instant, so correct once more around daylight saving changes.
  let instant = guess - zoneOffset(new Date(guess));
  instant = guess - zoneOffset(new Date(instant));
  const date = new Date(instant);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}
