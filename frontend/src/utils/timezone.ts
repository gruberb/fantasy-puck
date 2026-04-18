/**
 * Default date for navigation links ("Today's Games") — today in
 * local time. Games for today exist in the NHL schedule all day, so
 * today is the right starting point for the games view.
 */
export function getFixedAnalysisDateString(): string {
  return toLocalDateString();
}

/**
 * Default date for the daily fantasy rankings table. Rankings only
 * populate after a game-day completes (scheduler runs 9am & 3pm UTC
 * against yesterday), so defaulting to today produces "No daily
 * rankings available" during every in-progress slate. Yesterday is
 * the most recent day with real data.
 */
export function getMostRecentRankingsDate(): string {
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  return toLocalDateString(yesterday);
}

/**
 * Converts a date to an ISO date string (YYYY-MM-DD) in user's local timezone
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

