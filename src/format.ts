export type DateFormat = "DD/MM/YYYY" | "MM/DD/YYYY" | "YYYY-MM-DD";
export type TimeFormat = "24h" | "12h";

export const DEFAULT_DATE_FORMAT: DateFormat = "DD/MM/YYYY";
export const DEFAULT_TIME_FORMAT: TimeFormat = "24h";

export function normalizeDateFormat(s: string | undefined | null): DateFormat {
  if (s === "MM/DD/YYYY" || s === "YYYY-MM-DD") return s;
  return DEFAULT_DATE_FORMAT;
}

export function normalizeTimeFormat(s: string | undefined | null): TimeFormat {
  if (s === "12h") return "12h";
  return DEFAULT_TIME_FORMAT;
}

/** Format an ISO date (`YYYY-MM-DD`) for display. */
export function formatDate(
  iso: string,
  fmt: DateFormat = DEFAULT_DATE_FORMAT
): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso.trim());
  if (!m) return iso;
  const [, y, mo, d] = m;
  switch (fmt) {
    case "MM/DD/YYYY":
      return `${mo}/${d}/${y}`;
    case "YYYY-MM-DD":
      return `${y}-${mo}-${d}`;
    default:
      return `${d}/${mo}/${y}`;
  }
}

/** Format `HH:MM` / `HHMM` for display. */
export function formatTime(
  time: string,
  fmt: TimeFormat = DEFAULT_TIME_FORMAT
): string {
  const t = time.trim();
  let hh: number;
  let mm: number;
  const colon = /^(\d{1,2}):(\d{2})(?::\d{2})?$/.exec(t);
  if (colon) {
    hh = Number(colon[1]);
    mm = Number(colon[2]);
  } else if (/^\d{4}$/.test(t)) {
    hh = Number(t.slice(0, 2));
    mm = Number(t.slice(2, 4));
  } else {
    return time;
  }
  if (hh > 23 || mm > 59) return time;
  if (fmt === "12h") {
    const ap = hh >= 12 ? "PM" : "AM";
    const h12 = hh % 12 || 12;
    return `${h12}:${String(mm).padStart(2, "0")} ${ap}`;
  }
  return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
}

export function formatDateTime(
  date: string,
  time: string,
  dateFmt: DateFormat = DEFAULT_DATE_FORMAT,
  timeFmt: TimeFormat = DEFAULT_TIME_FORMAT
): string {
  const d = formatDate(date, dateFmt);
  const t = time ? formatTime(time, timeFmt) : "";
  return t ? `${d} ${t}` : d;
}
