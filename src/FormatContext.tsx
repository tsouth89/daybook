import { createContext, useContext } from "react";
import {
  DEFAULT_DATE_FORMAT,
  DEFAULT_TIME_FORMAT,
  formatDate,
  formatDateTime,
  formatTime,
  normalizeDateFormat,
  normalizeTimeFormat,
  type DateFormat,
  type TimeFormat,
} from "./format";

export type FormatPrefs = {
  dateFormat: DateFormat;
  timeFormat: TimeFormat;
  date: (iso: string) => string;
  time: (t: string) => string;
  dateTime: (date: string, time: string) => string;
};

const defaultPrefs: FormatPrefs = {
  dateFormat: DEFAULT_DATE_FORMAT,
  timeFormat: DEFAULT_TIME_FORMAT,
  date: (iso) => formatDate(iso, DEFAULT_DATE_FORMAT),
  time: (t) => formatTime(t, DEFAULT_TIME_FORMAT),
  dateTime: (d, t) => formatDateTime(d, t, DEFAULT_DATE_FORMAT, DEFAULT_TIME_FORMAT),
};

const FormatContext = createContext<FormatPrefs>(defaultPrefs);

export function FormatProvider({
  dateFormat,
  timeFormat,
  children,
}: {
  dateFormat?: string;
  timeFormat?: string;
  children: React.ReactNode;
}) {
  const df = normalizeDateFormat(dateFormat);
  const tf = normalizeTimeFormat(timeFormat);
  const value: FormatPrefs = {
    dateFormat: df,
    timeFormat: tf,
    date: (iso) => formatDate(iso, df),
    time: (t) => formatTime(t, tf),
    dateTime: (d, t) => formatDateTime(d, t, df, tf),
  };
  return <FormatContext.Provider value={value}>{children}</FormatContext.Provider>;
}

export function useFormat(): FormatPrefs {
  return useContext(FormatContext);
}
