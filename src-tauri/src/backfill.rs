//! Recover the item layer from markdown that was written before it existed.
//!
//! Everything in the vault outside `raw/` is a build artifact, but re-deriving
//! entries from `raw/` would mean re-triaging every capture through the model.
//! The rendered markdown is regular enough to parse back instead — entity pages
//! carry `### \`item_id\`` markers and `**Accomplished**` / `**Decided**` /
//! `**Open**` blocks — so backfill costs nothing.
//!
//! Fidelity is lower than a real triage pass: prose that was merged into a
//! section cannot be split back into the entries that produced it. Recovered
//! records are tagged `recovered` so a rebuild can redo them without ever
//! disturbing a record written at capture time.

use crate::entries::{EntryRecord, SOURCE_RECOVERED};
use crate::vault;
use anyhow::Result;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Default, Serialize)]
pub struct RebuildReport {
    /// Records recovered from markdown this run.
    pub recovered: usize,
    /// Capture-time records left untouched.
    pub kept: usize,
    /// Legacy task lines given an id marker so their checkbox can be tracked.
    pub tasks_marked: usize,
}

/// Rebuild the recovered half of the index from what is already in the vault.
pub fn rebuild(v: &Path, date_fmt: &str) -> Result<RebuildReport> {
    let all = crate::entries::load(v);
    let kept: Vec<EntryRecord> = all
        .into_iter()
        .filter(|r| r.source != SOURCE_RECOVERED)
        .collect();
    // Captures that already have real records are left alone entirely.
    let covered: HashSet<String> = kept.iter().map(|r| r.item_id.clone()).collect();
    // Record ids, for the markdown that carries them inline (tasks, ideas).
    let kept_ids: HashSet<String> = kept.iter().map(|r| r.id.clone()).collect();

    let mut out: Vec<EntryRecord> = Vec::new();
    let meta = vault::read_projects_config(v);

    for (kind, dir) in [
        ("project", vault::projects_dir(v)),
        ("area", vault::areas_dir(v)),
    ] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let slug = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let m = meta.iter().find(|m| m.slug == slug);
            let name = m
                .map(|m| m.name.clone())
                .unwrap_or_else(|| page_title(&text).unwrap_or_else(|| slug.clone()));
            let scope = m.map(|m| m.scope.clone()).unwrap_or_default();
            out.extend(parse_entity_page(
                &text, kind, &slug, &name, &scope, date_fmt, &covered,
            ));
        }
    }

    let tasks_marked = recover_tasks(v, date_fmt, &kept_ids, &mut out)?;
    recover_ideas(v, date_fmt, &kept_ids, &mut out);
    recover_day_notes(v, &covered, &mut out);

    let recovered = out.len();
    let mut merged = kept;
    let kept_count = merged.len();
    merged.extend(out);
    merged.sort_by(|a, b| (&a.date, &a.time, &a.id).cmp(&(&b.date, &b.time, &b.id)));
    crate::entries::write_all_records(v, &merged)?;

    Ok(RebuildReport {
        recovered,
        kept: kept_count,
        tasks_marked,
    })
}

fn page_title(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
}

fn record(
    id: String,
    item_id: String,
    date: String,
    time: String,
    scope: &str,
    kind: &str,
    slug: &str,
    name: &str,
    title: String,
) -> EntryRecord {
    EntryRecord {
        id,
        item_id,
        date,
        time,
        scope: if scope.is_empty() { "personal" } else { scope }.to_string(),
        kind: kind.to_string(),
        slug: slug.to_string(),
        name: name.to_string(),
        title,
        body: String::new(),
        accomplished: vec![],
        decisions: vec![],
        open: vec![],
        due: None,
        source: SOURCE_RECOVERED.into(),
    }
}

// ------------------------------------------------------------ entity pages

fn parse_entity_page(
    text: &str,
    kind: &str,
    slug: &str,
    name: &str,
    scope: &str,
    date_fmt: &str,
    covered: &HashSet<String>,
) -> Vec<EntryRecord> {
    let mut out = Vec::new();
    let mut cur_date: Option<String> = None;
    let mut cur_item: Option<String> = None;
    let mut body = String::new();
    let mut n = 0usize;

    let flush = |date: &Option<String>,
                     item: &mut Option<String>,
                     body: &mut String,
                     out: &mut Vec<EntryRecord>,
                     n: &mut usize| {
        let (Some(date), Some(item_id)) = (date.clone(), item.take()) else {
            body.clear();
            return;
        };
        if item_id == "_legacy" || covered.contains(&item_id) {
            body.clear();
            return;
        }
        let parsed = parse_section_body(body);
        body.clear();
        let mut r = record(
            format!("{item_id}-r{n}"),
            item_id,
            date,
            String::new(),
            scope,
            kind,
            slug,
            name,
            parsed.title,
        );
        r.accomplished = parsed.accomplished;
        r.decisions = parsed.decisions;
        r.open = parsed.open;
        r.body = parsed.rest;
        *n += 1;
        out.push(r);
    };

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some(iso) = crate::datetime::parse_date_to_iso(rest.trim(), date_fmt) {
                flush(&cur_date, &mut cur_item, &mut body, &mut out, &mut n);
                cur_date = Some(iso);
                continue;
            }
            // A non-date `##` (Overview, or something hand-written) ends the log.
            flush(&cur_date, &mut cur_item, &mut body, &mut out, &mut n);
            cur_date = None;
            continue;
        }
        if cur_date.is_none() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("### `") {
            if let Some((id, _)) = rest.split_once('`') {
                flush(&cur_date, &mut cur_item, &mut body, &mut out, &mut n);
                cur_item = Some(id.to_string());
                continue;
            }
        }
        body.push_str(line);
        body.push('\n');
    }
    flush(&cur_date, &mut cur_item, &mut body, &mut out, &mut n);
    out
}

#[derive(Default)]
struct ParsedSection {
    title: String,
    accomplished: Vec<String>,
    decisions: Vec<String>,
    open: Vec<String>,
    rest: String,
}

/// Reverse of `ai::render_entity_section`.
fn parse_section_body(body: &str) -> ParsedSection {
    #[derive(PartialEq)]
    enum Mode {
        None,
        Acc,
        Dec,
        Open,
    }
    let mut p = ParsedSection::default();
    let mut mode = Mode::None;
    let mut rest = String::new();

    for line in body.lines() {
        let t = line.trim();
        match t {
            "**Accomplished**" => {
                mode = Mode::Acc;
                continue;
            }
            "**Decided**" => {
                mode = Mode::Dec;
                continue;
            }
            "**Open**" => {
                mode = Mode::Open;
                continue;
            }
            _ => {}
        }
        if t.is_empty() {
            if mode == Mode::None {
                rest.push('\n');
            }
            continue;
        }
        if let Some(item) = t.strip_prefix("- ") {
            match mode {
                Mode::Acc => {
                    p.accomplished.push(item.trim().to_string());
                    continue;
                }
                Mode::Dec => {
                    p.decisions.push(item.trim().to_string());
                    continue;
                }
                Mode::Open => {
                    p.open.push(item.trim().to_string());
                    continue;
                }
                Mode::None => {}
            }
        } else {
            mode = Mode::None;
        }
        // The renderer's trailing day link is noise on the way back.
        if t.starts_with("[[days/") {
            continue;
        }
        if p.title.is_empty() && rest.trim().is_empty() {
            if let Some(inner) = t.strip_prefix("**").and_then(|s| s.strip_suffix("**")) {
                p.title = inner.trim().to_string();
                continue;
            }
        }
        rest.push_str(line);
        rest.push('\n');
    }
    p.rest = rest.trim().to_string();
    if p.title.is_empty() {
        p.title = p
            .rest
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .take(80)
            .collect();
    }
    p
}

// ------------------------------------------------------------------- tasks

/// Give legacy task lines an id marker so their checkbox becomes trackable,
/// and recover a record for each. Lines that already carry a marker are left
/// exactly as they are.
fn recover_tasks(
    v: &Path,
    date_fmt: &str,
    kept_ids: &HashSet<String>,
    out: &mut Vec<EntryRecord>,
) -> Result<usize> {
    let path = vault::tasks_path(v);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(0);
    };
    let mut marked = 0usize;
    let mut lines: Vec<String> = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    let mut n = 0usize;

    for line in text.lines() {
        let t = line.trim_start();
        let is_task = t.starts_with("- [ ]") || t.starts_with("- [x]") || t.starts_with("- [X]");
        if !is_task {
            lines.push(line.to_string());
            continue;
        }

        // A marker written by the forward path belongs to a real record; leave it.
        // A marker this backfill wrote before must be re-derived, because the
        // recovered half of the index is rebuilt from scratch every run.
        let existing = crate::entries::marker_id_of(t);
        if let Some(id) = &existing {
            if kept_ids.contains(id) {
                lines.push(line.to_string());
                continue;
            }
        }

        let id = existing.unwrap_or_else(|| {
            loop {
                let candidate = format!("recovered-task-{n}");
                n += 1;
                if !used.contains(&candidate) {
                    return candidate;
                }
            }
        });
        used.insert(id.clone());

        let parsed = parse_task_line(t, date_fmt);
        let mut r = record(
            id.clone(),
            parsed.item_id.clone(),
            parsed.date.clone(),
            String::new(),
            &parsed.scope,
            "task",
            &parsed.slug,
            "",
            parsed.text.clone(),
        );
        r.due = parsed.due.clone();
        out.push(r);

        if t.contains("<!-- e:") {
            lines.push(line.to_string());
        } else {
            // Insert the marker straight after the checkbox.
            let rebuilt = t.replacen("] ", &format!("] <!-- e:{id} --> "), 1);
            let indent = &line[..line.len() - t.len()];
            lines.push(format!("{indent}{rebuilt}"));
            marked += 1;
        }
    }

    if marked > 0 {
        let mut joined = lines.join("\n");
        if !joined.ends_with('\n') {
            joined.push('\n');
        }
        std::fs::write(&path, joined)?;
    }
    Ok(marked)
}

#[derive(Default)]
struct ParsedTask {
    item_id: String,
    scope: String,
    text: String,
    date: String,
    due: Option<String>,
    slug: String,
}

fn parse_task_line(line: &str, date_fmt: &str) -> ParsedTask {
    let mut p = ParsedTask::default();
    let body = line
        .trim_start_matches("- [ ]")
        .trim_start_matches("- [x]")
        .trim_start_matches("- [X]")
        .trim();

    let mut rest = body;
    if let Some(close) = rest.find(')') {
        if rest.starts_with('(') {
            p.scope = rest[1..close].trim().to_string();
            rest = rest[close + 1..].trim();
        }
    }
    p.slug = wikilink_slug_opt(rest).unwrap_or_default();
    // `text — captured DD/MM/YYYY · due DD/MM/YYYY · [[link]]`
    let (text, tail) = match rest.split_once(" — captured ") {
        Some((a, b)) => (a, b),
        None => (rest, ""),
    };
    p.text = strip_trailing_link(text).trim().to_string();
    for (i, part) in tail.split('·').enumerate() {
        let part = part.trim();
        if i == 0 {
            p.date = crate::datetime::parse_date_to_iso(part, date_fmt).unwrap_or_default();
        } else if let Some(d) = part.strip_prefix("due ") {
            p.due = crate::datetime::parse_date_to_iso(d.trim(), date_fmt);
        }
    }
    if p.date.is_empty() {
        p.date = vault::today();
    }
    p.item_id = format!("recovered-tasks-{}", p.date);
    p
}

/// Pull the slug out of a trailing `[[projects/slug]]` / `[[areas/slug]]` link.
fn wikilink_slug_opt(s: &str) -> Option<String> {
    let start = s.find("[[")? + 2;
    let rest = &s[start..];
    let end = rest.find("]]")?;
    let target = rest[..end].split('|').next().unwrap_or("").trim();
    let slug = target.rsplit('/').next().unwrap_or("").trim();
    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

fn strip_trailing_link(s: &str) -> &str {
    match s.find(" · [[") {
        Some(i) => &s[..i],
        None => s,
    }
}

// ------------------------------------------------------------------- ideas

fn recover_ideas(
    v: &Path,
    date_fmt: &str,
    covered_ids: &HashSet<String>,
    out: &mut Vec<EntryRecord>,
) {
    let Ok(text) = std::fs::read_to_string(vault::ideas_path(v)) else {
        return;
    };
    let mut cur_date: Option<String> = None;
    let mut n = 0usize;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            cur_date = crate::datetime::parse_date_to_iso(rest.trim(), date_fmt);
            continue;
        }
        let Some(date) = cur_date.clone() else {
            continue;
        };
        let t = line.trim();
        if !t.starts_with("- ") {
            continue;
        }
        // Marked bullets already have a record from the forward path.
        if let Some(id) = crate::entries::marker_id_of(t) {
            if covered_ids.contains(&id) {
                continue;
            }
        }
        let item_id = format!("recovered-ideas-{date}");
        let mut body = t[2..].trim();
        let mut time = String::new();
        if let Some(rest) = body.strip_prefix("**") {
            if let Some((t0, r)) = rest.split_once("**") {
                time = t0.trim().to_string();
                body = r.trim();
            }
        }
        if let Some(after) = body.find("-->") {
            if body.starts_with("<!--") {
                body = body[after + 3..].trim();
            }
        }
        let mut scope = String::new();
        if body.starts_with('(') {
            if let Some(close) = body.find(')') {
                scope = body[1..close].trim().to_string();
                body = body[close + 1..].trim();
            }
        }
        let slug = wikilink_slug_opt(body).unwrap_or_default();
        let text = strip_trailing_link(body).trim().to_string();
        let mut r = record(
            format!("recovered-idea-{n}"),
            item_id,
            date,
            time,
            &scope,
            "idea",
            &slug,
            "",
            text.chars().take(120).collect(),
        );
        r.body = text;
        out.push(r);
        n += 1;
    }
}

// --------------------------------------------------------------- day notes

/// Captures whose only trace is the day note — pure `note` entries.
fn recover_day_notes(v: &Path, covered: &HashSet<String>, out: &mut Vec<EntryRecord>) {
    let already: HashSet<String> = out.iter().map(|r| r.item_id.clone()).collect();
    let Ok(rd) = std::fs::read_dir(vault::days_dir(v)) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let date = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if vault::valid_date(&date).is_err() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("## ") else {
                continue;
            };
            let Some((time, rest)) = rest.split_once(" · ") else {
                continue;
            };
            let Some((id_bit, title)) = rest.split_once(" · ") else {
                continue;
            };
            let item_id = id_bit.trim().trim_matches('`').to_string();
            if covered.contains(&item_id) || already.contains(&item_id) {
                continue;
            }
            out.push(record(
                format!("{item_id}-r-note"),
                item_id,
                date.clone(),
                time.trim().to_string(),
                "",
                "note",
                "",
                "",
                title.trim().to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries;

    fn tmp() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "daybook-backfill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        crate::vault::ensure_vault(&p).unwrap();
        p
    }

    /// Exactly what `ai::render_entity_section` produces, wrapped in a page.
    const PROJECT_PAGE: &str = "\
---
slug: daybook
---

# Daybook

## Overview

Hand-written standing summary that must not become an entry.

## 06/08/2026

### `cap-1`

**Inbox routing**

**Accomplished**

- Wired inbox triage

**Decided**

- Kept raw append-only, because rebuilds have to be safe

**Open**

- Decide on the index format

Got the inbox layer wiring so captures split into discrete entries.

[[days/2026-08-06]]
";

    #[test]
    fn recovers_structure_from_a_rendered_project_page() {
        let v = tmp();
        std::fs::write(crate::vault::projects_dir(&v).join("daybook.md"), PROJECT_PAGE).unwrap();

        let report = rebuild(&v, "DD/MM/YYYY").unwrap();
        assert_eq!(report.recovered, 1);

        let all = entries::load(&v);
        let r = &all[0];
        assert_eq!(r.item_id, "cap-1");
        assert_eq!(r.kind, "project");
        assert_eq!(r.slug, "daybook");
        assert_eq!(r.date, "2026-08-06");
        assert_eq!(r.title, "Inbox routing");
        assert_eq!(r.accomplished, vec!["Wired inbox triage"]);
        assert_eq!(
            r.decisions,
            vec!["Kept raw append-only, because rebuilds have to be safe"]
        );
        assert_eq!(r.open, vec!["Decide on the index format"]);
        assert!(r.body.contains("Got the inbox layer wiring"));
        // The trailing day link is noise, not body.
        assert!(!r.body.contains("[[days/"));
        assert_eq!(r.source, SOURCE_RECOVERED);
    }

    #[test]
    fn hand_written_overview_never_becomes_an_entry() {
        let v = tmp();
        std::fs::write(crate::vault::projects_dir(&v).join("daybook.md"), PROJECT_PAGE).unwrap();
        rebuild(&v, "DD/MM/YYYY").unwrap();
        let all = entries::load(&v);
        assert_eq!(all.len(), 1);
        assert!(!all.iter().any(|r| r.body.contains("Hand-written standing")));
    }

    #[test]
    fn rebuilding_twice_is_idempotent() {
        let v = tmp();
        std::fs::write(crate::vault::projects_dir(&v).join("daybook.md"), PROJECT_PAGE).unwrap();
        std::fs::write(
            crate::vault::tasks_path(&v),
            "# Tasks\n\n- [ ] (work) Write tests — captured 06/08/2026\n",
        )
        .unwrap();

        let first = rebuild(&v, "DD/MM/YYYY").unwrap();
        let after_first = entries::load(&v).len();
        let second = rebuild(&v, "DD/MM/YYYY").unwrap();
        assert_eq!(entries::load(&v).len(), after_first);
        assert_eq!(first.recovered, second.recovered);
        // The marker is written once; the second pass finds it already there.
        assert_eq!(first.tasks_marked, 1);
        assert_eq!(second.tasks_marked, 0);
    }

    #[test]
    fn capture_time_records_are_never_disturbed() {
        let v = tmp();
        std::fs::write(crate::vault::projects_dir(&v).join("daybook.md"), PROJECT_PAGE).unwrap();

        // A real triage record for the same capture the page came from.
        let real = entries::EntryRecord {
            id: "cap-1-e0".into(),
            item_id: "cap-1".into(),
            date: "2026-08-06".into(),
            time: "09:30".into(),
            scope: "work".into(),
            kind: "project".into(),
            slug: "daybook".into(),
            name: "Daybook".into(),
            title: "The real one".into(),
            body: String::new(),
            accomplished: vec![],
            decisions: vec![],
            open: vec![],
            due: None,
            source: entries::SOURCE_TRIAGE.into(),
        };
        entries::replace_item(&v, "cap-1", &[real]).unwrap();

        let report = rebuild(&v, "DD/MM/YYYY").unwrap();
        // Nothing recovered: that capture is already covered by a real record.
        assert_eq!(report.recovered, 0);
        assert_eq!(report.kept, 1);
        let all = entries::load(&v);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "The real one");
    }

    #[test]
    fn legacy_tasks_get_markers_and_become_trackable() {
        let v = tmp();
        std::fs::write(
            crate::vault::tasks_path(&v),
            "# Tasks\n\n\
             - [ ] (work) Write tests — captured 06/08/2026 · due 12/08/2026 · [[projects/daybook]]\n\
             - [x] (personal) Book dentist — captured 01/08/2026\n",
        )
        .unwrap();

        let report = rebuild(&v, "DD/MM/YYYY").unwrap();
        assert_eq!(report.tasks_marked, 2);

        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("<!-- e:recovered-task-0 -->"), "got: {text}");
        // The original wording survives the migration.
        assert!(text.contains("Write tests — captured 06/08/2026"));

        let all = entries::load(&v);
        let first = all.iter().find(|r| r.id == "recovered-task-0").unwrap();
        assert_eq!(first.kind, "task");
        assert_eq!(first.slug, "daybook");
        assert_eq!(first.due.as_deref(), Some("2026-08-12"));
        assert_eq!(first.date, "2026-08-06");
        assert_eq!(first.title, "Write tests");

        // The ticked one is read back as done through its new marker.
        let done = entries::task_state(&v);
        assert_eq!(done.get("recovered-task-1"), Some(&true));
        assert_eq!(done.get("recovered-task-0"), Some(&false));
    }

    #[test]
    fn ideas_and_day_notes_are_recovered() {
        let v = tmp();
        std::fs::write(
            crate::vault::ideas_path(&v),
            "# Ideas\n\n## 06/08/2026\n\n- **09:30** (work) Ship a CLI · [[projects/daybook]]\n",
        )
        .unwrap();
        std::fs::write(
            crate::vault::days_dir(&v).join("2026-08-06.md"),
            "# 06/08/2026\n\n## 11:00 · `cap-9` · A passing thought\n\nBody.\n",
        )
        .unwrap();

        rebuild(&v, "DD/MM/YYYY").unwrap();
        let all = entries::load(&v);

        let idea = all.iter().find(|r| r.kind == "idea").unwrap();
        assert_eq!(idea.slug, "daybook");
        assert_eq!(idea.title, "Ship a CLI");
        assert_eq!(idea.time, "09:30");

        let note = all.iter().find(|r| r.kind == "note").unwrap();
        assert_eq!(note.item_id, "cap-9");
        assert_eq!(note.title, "A passing thought");
    }
}
