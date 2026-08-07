//! Display date/time formatting. Vault paths and ids stay ISO (`YYYY-MM-DD`);
//! human-visible stamps in the UI and note bodies follow Settings.

use chrono::{NaiveDate, NaiveTime};

pub const DEFAULT_DATE_FORMAT: &str = "DD/MM/YYYY";
pub const DEFAULT_TIME_FORMAT: &str = "24h";

/// What a fresh install should use. Guessing one region's convention for
/// everybody guarantees it reads wrong for most of them, so ask the OS.
pub fn locale_date_format() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    // Locales that write the big end first.
    if ["sv", "lt", "hu", "ja", "zh", "ko"]
        .iter()
        .any(|p| locale.starts_with(p))
    {
        return "YYYY-MM-DD".into();
    }
    if locale.starts_with("en-us") || locale.starts_with("en_us") {
        return "MM/DD/YYYY".into();
    }
    DEFAULT_DATE_FORMAT.into()
}

pub fn locale_time_format() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    // 12-hour clock is mostly an anglophone habit outside the UK.
    if ["en-us", "en_us", "en-ca", "en_ca", "en-au", "en_au", "en-nz", "en_nz", "en-ph", "en_ph"]
        .iter()
        .any(|p| locale.starts_with(p))
    {
        return "12h".into();
    }
    DEFAULT_TIME_FORMAT.into()
}

pub fn normalize_date_format(s: &str) -> &str {
    match s.trim() {
        "MM/DD/YYYY" => "MM/DD/YYYY",
        "YYYY-MM-DD" => "YYYY-MM-DD",
        _ => DEFAULT_DATE_FORMAT,
    }
}

pub fn normalize_time_format(s: &str) -> &str {
    match s.trim().to_lowercase().as_str() {
        "12h" | "12" | "ampm" => "12h",
        _ => DEFAULT_TIME_FORMAT,
    }
}

/// Format an ISO date (`YYYY-MM-DD`) for display. Unknown input is returned as-is.
pub fn format_date(iso: &str, fmt: &str) -> String {
    let Ok(d) = NaiveDate::parse_from_str(iso.trim(), "%Y-%m-%d") else {
        return iso.to_string();
    };
    match normalize_date_format(fmt) {
        "MM/DD/YYYY" => d.format("%m/%d/%Y").to_string(),
        "YYYY-MM-DD" => d.format("%Y-%m-%d").to_string(),
        _ => d.format("%d/%m/%Y").to_string(),
    }
}

/// Format a time like `14:30` or `1430`. Unknown input is returned as-is.
pub fn format_time(time: &str, fmt: &str) -> String {
    let Some(t) = parse_time(time) else {
        return time.to_string();
    };
    match normalize_time_format(fmt) {
        "12h" => t.format("%I:%M %p").to_string().trim_start_matches('0').to_string(),
        _ => t.format("%H:%M").to_string(),
    }
}

fn parse_time(time: &str) -> Option<NaiveTime> {
    let t = time.trim();
    if let Ok(v) = NaiveTime::parse_from_str(t, "%H:%M") {
        return Some(v);
    }
    if let Ok(v) = NaiveTime::parse_from_str(t, "%H:%M:%S") {
        return Some(v);
    }
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        let hh: u32 = t[0..2].parse().ok()?;
        let mm: u32 = t[2..4].parse().ok()?;
        return NaiveTime::from_hms_opt(hh, mm, 0);
    }
    None
}

/// Parse a human or ISO date into ISO `YYYY-MM-DD`, preferring the user's format.
pub fn parse_date_to_iso(s: &str, preferred_fmt: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let preferred = normalize_date_format(preferred_fmt);
    let patterns: &[&str] = match preferred {
        "MM/DD/YYYY" => &["%m/%d/%Y", "%Y-%m-%d", "%d/%m/%Y"],
        "YYYY-MM-DD" => &["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y"],
        _ => &["%d/%m/%Y", "%Y-%m-%d", "%m/%d/%Y"],
    };
    for p in patterns {
        if let Ok(d) = NaiveDate::parse_from_str(s, p) {
            return Some(d.format("%Y-%m-%d").to_string());
        }
    }
    None
}
