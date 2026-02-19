const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/**
 * Format an ISO-ish timestamp (e.g. "2026-06-22T13:16:30" or "2026-06-22T13:16:30.000")
 * into a human-readable form: "Jun 22, 13:16:30".
 * Omits the year (always current-year assumption for log analysis).
 * Returns the original string if parsing fails.
 */
export function formatTimestamp(iso: string): string {
  if (!iso) return '';
  // Append Z if no timezone info to parse as UTC
  const normalized = iso.includes('Z') || iso.includes('+') ? iso : iso + 'Z';
  const d = new Date(normalized);
  if (isNaN(d.getTime())) return iso;
  const mon = MONTHS[d.getUTCMonth()];
  const day = d.getUTCDate();
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  const ss = String(d.getUTCSeconds()).padStart(2, '0');
  return `${mon} ${day}, ${hh}:${mm}:${ss}`;
}

/** Format a UTC epoch (ms) into "Jun 22, 13:16:30". */
export function formatTimestampMs(ms: number): string {
  const d = new Date(ms);
  if (isNaN(d.getTime())) return String(ms);
  const mon = MONTHS[d.getUTCMonth()];
  const day = d.getUTCDate();
  const hh = String(d.getUTCHours()).padStart(2, '0');
  const mm = String(d.getUTCMinutes()).padStart(2, '0');
  const ss = String(d.getUTCSeconds()).padStart(2, '0');
  return `${mon} ${day}, ${hh}:${mm}:${ss}`;
}

/** Format a datetime-local input value (local time) into "Jun 22, 13:16". */
export function formatDatetimeLocal(dt: string): string {
  const d = new Date(dt);
  if (isNaN(d.getTime())) return dt;
  const mon = MONTHS[d.getMonth()];
  const day = d.getDate();
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${mon} ${day}, ${hh}:${mm}`;
}
