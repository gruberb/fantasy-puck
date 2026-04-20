/**
 * The NHL keys its schedule by Eastern Time, and so do we: the backend's
 * `hockey_today()` (see `backend/src/api/handlers/insights.rs`) returns the
 * ET calendar date, and every date-scoped endpoint — games, pulse, rankings,
 * race-odds, insights — is keyed against it. The frontend MUST match, or
 * viewers outside ET see the wrong slate as soon as UTC or their browser
 * crosses midnight before the ET game night is over.
 *
 * Use `getHockeyDateToday()` / `getHockeyDateYesterday()` for any "today"
 * or "yesterday" derivation. Never reach for `new Date().toISOString()` or
 * `toLocaleDateString()` in shipped code — both drift from ET.
 */

const HOCKEY_DATE_FORMATTER = new Intl.DateTimeFormat("en-CA", {
  timeZone: "America/New_York",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});

/**
 * YYYY-MM-DD for "today" in Eastern Time — the date the NHL's schedule
 * endpoint would use right now. A viewer in London at 03:00 BST while Apr 19
 * ET games are still live gets `2026-04-19`, matching the backend.
 */
export function getHockeyDateToday(): string {
  return HOCKEY_DATE_FORMATTER.format(new Date());
}

/**
 * YYYY-MM-DD for the previous ET calendar day — used to default the
 * Daily Rankings view, since the rankings scheduler populates each day's
 * numbers only after the slate completes.
 */
export function getHockeyDateYesterday(): string {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() - 1);
  return HOCKEY_DATE_FORMATTER.format(d);
}

/**
 * YYYY-MM-DD for a `Date` object the way the user's browser sees it. This
 * exists for date-picker round-tripping: a `<DateHeader>` / calendar widget
 * hands us a `Date` pinned to local midnight of the user-chosen day, and we
 * turn it back into a string. Do not use this to derive "today" — use
 * `getHockeyDateToday()` for that.
 */
export function toLocalDateString(date: Date = new Date()): string {
  return date.toLocaleDateString("en-CA"); // en-CA returns YYYY-MM-DD format
}

/**
 * Formats a date for display with the user's timezone
 */
export function formatDisplayDate(
  date: Date,
  options: Intl.DateTimeFormatOptions = {},
): string {
  const defaultOptions: Intl.DateTimeFormatOptions = {
    weekday: "long",
    month: "long",
    day: "numeric",
    year: "numeric",
  };

  return date.toLocaleDateString("en-US", { ...defaultOptions, ...options });
}

/**
 * Creates a Date object from a YYYY-MM-DD string in the user's timezone
 */
export function dateStringToLocalDate(dateString: string): Date {
  const [year, month, day] = dateString.split("-").map(Number);
  return new Date(year, month - 1, day); // month is 0-indexed in JS Date
}

/**
 * Compare if two dates are the same day in the user's local timezone
 */
export function isSameLocalDay(date1: Date, date2: Date): boolean {
  return (
    date1.getFullYear() === date2.getFullYear() &&
    date1.getMonth() === date2.getMonth() &&
    date1.getDate() === date2.getDate()
  );
}
