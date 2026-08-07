use anyhow::{Context, Result};
use base64::Engine;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Layout
//
//   inbox/                   ingress. Anything may drop a file here.
//   raw/YYYY-MM-DD.md        append-only archive of triaged items. Source of truth.
//   days/YYYY-MM-DD.md       generated view over a day's entries
//   projects/<slug>.md       things with an end state
//   areas/<slug>.md          ongoing responsibilities with no end state
//   ideas.md                 maybe-someday, dated
//   tasks.md                 open checkboxes, dated
//   personal.md              rollup of personal-scoped entries over time
//   attachments/             pasted images
//   config/projects.json     known projects/areas + aliases (entity resolution)
//   config/glossary.txt      jargon list, used to repair dictation errors
//   config/profile.md        durable facts about the author, fed to every pass
//
// Only inbox/ and raw/ hold original text. Everything else is a build artifact
// and can be deleted and regenerated from raw/.
// ---------------------------------------------------------------------------

pub fn inbox_dir(v: &Path) -> PathBuf {
    v.join("inbox")
}
pub fn raw_dir(v: &Path) -> PathBuf {
    v.join("raw")
}
pub fn days_dir(v: &Path) -> PathBuf {
    v.join("days")
}
pub fn projects_dir(v: &Path) -> PathBuf {
    v.join("projects")
}
pub fn areas_dir(v: &Path) -> PathBuf {
    v.join("areas")
}
pub fn attachments_dir(v: &Path) -> PathBuf {
    v.join("attachments")
}
pub fn config_dir(v: &Path) -> PathBuf {
    v.join("config")
}

/// Where a given entry kind gets filed. `None` means it lives only in the day note.
pub fn dir_for_kind(v: &Path, kind: &str) -> Option<PathBuf> {
    match kind {
        "project" => Some(projects_dir(v)),
        "area" => Some(areas_dir(v)),
        _ => None,
    }
}

pub fn ensure_vault(v: &Path) -> Result<()> {
    for d in [
        inbox_dir(v),
        raw_dir(v),
        days_dir(v),
        projects_dir(v),
        areas_dir(v),
        attachments_dir(v),
        config_dir(v),
    ] {
        std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
    }

    let profile = config_dir(v).join("profile.md");
    if !profile.exists() {
        std::fs::write(
            &profile,
            "# Profile\n\n\
             Durable facts about you, sent with every pass so you never have to re-explain\n\
             yourself. Keep it short and factual; this is context, not an essay.\n\n\
             ## Who\n\n- \n\n## How I work\n\n- \n\n## Standing preferences\n\n- \n\n\
             ## People and tools I mention\n\n- \n",
        )?;
    }

    let projects_json = config_dir(v).join("projects.json");
    if !projects_json.exists() {
        std::fs::write(&projects_json, "[]\n")?;
    }

    let glossary = config_dir(v).join("glossary.txt");
    if !glossary.exists() {
        std::fs::write(
            &glossary,
            "# One term per line. These are fed to the model so it can repair\n\
             # dictation errors (\"tool port\" -> \"Toolport\").\n\
             # Lines starting with # are ignored.\n",
        )?;
    }

    let readme = v.join("README.md");
    if !readme.exists() {
        std::fs::write(
            &readme,
            "# Daybook vault\n\n\
             Capture drops into `inbox/`. After triage, the verbatim text is archived in `raw/`\n\
             (append-only, never edited by the AI) and routed copies land in `projects/`, `areas/`,\n\
             `personal.md`, `ideas.md`, `tasks.md`, and `days/`.\n\n\
             Only `inbox/` and `raw/` hold original text. Everything else is a build artifact and\n\
             can be deleted and regenerated. This folder is a valid Obsidian vault — open it\n\
             directly for graph view, backlinks, or hand-written notes in the right place.\n",
        )?;
    }

    for (name, heading) in [
        ("ideas.md", "# Ideas\n\n"),
        ("tasks.md", "# Tasks\n\n"),
        (
            "personal.md",
            "# Personal\n\n\
             Life notes and personal-scoped entries over time. Project work that happens to be\n\
             personal-scoped also appears here as a short pointer; the full log lives on the\n\
             project page.\n\n",
        ),
    ] {
        let path = v.join(name);
        if !path.exists() {
            std::fs::write(&path, heading)?;
        }
    }
    Ok(())
}

pub fn read_profile(v: &Path) -> String {
    std::fs::read_to_string(config_dir(v).join("profile.md")).unwrap_or_default()
}

pub fn write_profile(v: &Path, text: &str) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(config_dir(v).join("profile.md"), text)?;
    Ok(())
}

pub fn ideas_path(v: &Path) -> PathBuf {
    v.join("ideas.md")
}
pub fn tasks_path(v: &Path) -> PathBuf {
    v.join("tasks.md")
}
pub fn personal_path(v: &Path) -> PathBuf {
    v.join("personal.md")
}

pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub fn valid_date(date: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .with_context(|| format!("'{date}' is not a YYYY-MM-DD date"))
}

// ------------------------------------------------------------------- inbox

#[derive(Debug, Clone, Serialize)]
pub struct InboxItem {
    /// File stem, e.g. "2026-08-06-1432-a3f1". Also the stable id.
    pub id: String,
    pub date: String,
    pub time: String,
    pub text: String,
    pub chars: usize,
}

fn short_id() -> String {
    // Enough entropy to avoid collisions inside one second; this is not a security id.
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:04x}", n % 0xffff)
}

/// Every capture becomes its own file. Discrete items are what make per-item
/// routing possible, and they let anything else (phone, email, a script) drop
/// work in without knowing anything about this app.
pub fn write_inbox_item(v: &Path, text: &str) -> Result<String> {
    ensure_vault(v)?;
    let now = Local::now();
    let id = format!("{}-{}", now.format("%Y-%m-%d-%H%M"), short_id());
    let path = inbox_dir(v).join(format!("{id}.md"));
    let body = format!(
        "---\ncaptured: {}\n---\n\n{}\n",
        now.format("%Y-%m-%dT%H:%M:%S"),
        text.trim()
    );
    std::fs::write(&path, body).with_context(|| format!("writing {}", path.display()))?;
    Ok(id)
}

fn parse_inbox_file(id: &str, contents: &str) -> InboxItem {
    // id looks like YYYY-MM-DD-HHMM-xxxx
    let date = id.get(0..10).unwrap_or("").to_string();
    let time = id
        .get(11..15)
        .map(|t| format!("{}:{}", &t[0..2], &t[2..4]))
        .unwrap_or_default();
    let text = contents
        .split_once("---\n\n")
        .map(|(_, rest)| rest)
        .unwrap_or(contents)
        .trim()
        .to_string();
    InboxItem {
        id: id.to_string(),
        date,
        time,
        chars: text.chars().count(),
        text,
    }
}

pub fn list_inbox(v: &Path) -> Result<Vec<InboxItem>> {
    let mut items = Vec::new();
    if let Ok(rd) = std::fs::read_dir(inbox_dir(v)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(id) = name.strip_suffix(".md") else {
                continue;
            };
            let Ok(contents) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            items.push(parse_inbox_file(id, &contents));
        }
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(items)
}

pub fn delete_inbox_item(v: &Path, id: &str) -> Result<()> {
    let path = inbox_dir(v).join(format!("{}.md", sanitize_id(id)));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Rewrite the body of an inbox item; keeps id / frontmatter capture stamp.
pub fn update_inbox_item(v: &Path, id: &str, text: &str) -> Result<()> {
    ensure_vault(v)?;
    let id = sanitize_id(id);
    let path = inbox_dir(v).join(format!("{id}.md"));
    if !path.exists() {
        anyhow::bail!("Inbox item not found: {id}");
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let captured = existing
        .lines()
        .find_map(|l| l.strip_prefix("captured: ").map(|s| s.trim().to_string()))
        .unwrap_or_else(|| Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());
    let body = format!(
        "---\ncaptured: {captured}\n---\n\n{}\n",
        text.trim()
    );
    std::fs::write(&path, body)?;
    Ok(())
}

/// Ensure a day note file exists (empty scaffold) so Today always has somewhere to land.
pub fn ensure_day(v: &Path, date: &str, date_fmt: &str) -> Result<String> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = days_dir(v).join(format!("{date}.md"));
    if !path.exists() {
        let display = crate::datetime::format_date(date, date_fmt);
        let body = format!(
            "---\ndate: {date}\ntype: daily\n---\n\n# {display}\n\n## At a glance\n\n- _(nothing filed yet)_\n\n---\n\nRaw: [[raw/{date}]]\n"
        );
        std::fs::write(&path, body)?;
    }
    Ok(std::fs::read_to_string(&path)?)
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

// --------------------------------------------------------------------- raw

/// Appends a timestamped block to a day's raw file. Never overwrites.
#[allow(dead_code)]
pub fn append_raw(v: &Path, date: &str, text: &str) -> Result<()> {
    append_raw_item(
        v,
        date,
        None,
        text,
        crate::datetime::DEFAULT_DATE_FORMAT,
        crate::datetime::DEFAULT_TIME_FORMAT,
    )
}

pub fn append_raw_item(
    v: &Path,
    date: &str,
    item_id: Option<&str>,
    text: &str,
    date_fmt: &str,
    time_fmt: &str,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = raw_dir(v).join(format!("{date}.md"));

    let mut out = String::new();
    if !path.exists() {
        let title = crate::datetime::format_date(date, date_fmt);
        out.push_str(&format!("# {title} (raw)\n\n"));
    }
    let stamp = crate::datetime::format_time(&Local::now().format("%H:%M").to_string(), time_fmt);
    match item_id {
        Some(id) => out.push_str(&format!("## {stamp} · `{id}`\n\n")),
        None => out.push_str(&format!("## {stamp}\n\n")),
    }
    out.push_str(&format!("{}\n\n", text.trim()));

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    f.write_all(out.as_bytes())?;
    Ok(())
}

pub fn read_raw(v: &Path, date: &str) -> Result<String> {
    valid_date(date)?;
    Ok(std::fs::read_to_string(raw_dir(v).join(format!("{date}.md"))).unwrap_or_default())
}

pub fn write_raw(v: &Path, date: &str, content: &str) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    std::fs::write(raw_dir(v).join(format!("{date}.md")), content)?;
    Ok(())
}

// -------------------------------------------------------------------- notes

pub fn read_note(v: &Path, date: &str) -> Result<String> {
    valid_date(date)?;
    Ok(std::fs::read_to_string(days_dir(v).join(format!("{date}.md"))).unwrap_or_default())
}

#[allow(dead_code)]
pub fn write_note(v: &Path, date: &str, content: &str) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    std::fs::write(days_dir(v).join(format!("{date}.md")), content)?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DayEntry {
    pub date: String,
    pub has_raw: bool,
    pub has_note: bool,
    pub raw_chars: usize,
    pub preview: String,
}

/// Every date that has either a raw dump or a generated note, newest first.
pub fn list_days(v: &Path) -> Result<Vec<DayEntry>> {
    let mut dates: Vec<String> = Vec::new();
    for dir in [raw_dir(v), days_dir(v)] {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if let Some(stem) = name.strip_suffix(".md") {
                    if valid_date(stem).is_ok() && !dates.iter().any(|d| d == stem) {
                        dates.push(stem.to_string());
                    }
                }
            }
        }
    }
    dates.sort();
    dates.reverse();

    let mut out = Vec::with_capacity(dates.len());
    for date in dates {
        let raw = read_raw(v, &date).unwrap_or_default();
        let note = read_note(v, &date).unwrap_or_default();
        let source = if note.is_empty() { &raw } else { &note };
        out.push(DayEntry {
            has_raw: !raw.is_empty(),
            has_note: !note.is_empty(),
            raw_chars: raw.chars().count(),
            preview: preview_of(source),
            date,
        });
    }
    Ok(out)
}

fn preview_of(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| {
            !l.is_empty()
                && !l.starts_with('#')
                && !l.starts_with("---")
                && !l.starts_with("date:")
                && !l.starts_with("projects:")
        })
        .unwrap_or("");
    let mut s: String = line.chars().take(160).collect();
    if line.chars().count() > 160 {
        s.push('…');
    }
    s
}

// ----------------------------------------------------------------- projects

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectMeta {
    pub slug: String,
    pub name: String,
    /// "project" (has an end state) or "area" (ongoing responsibility).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// "personal" or "work". Orthogonal to kind: a personal project is both.
    #[serde(default = "default_scope")]
    pub scope: String,
    /// `active` | `paused` | `done`. Only projects really finish; areas stay active.
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: String,
}

fn default_status() -> String {
    "active".into()
}

fn default_kind() -> String {
    "project".into()
}
fn default_scope() -> String {
    "work".into()
}

pub fn read_projects_config(v: &Path) -> Vec<ProjectMeta> {
    std::fs::read_to_string(config_dir(v).join("projects.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<ProjectMeta>>(&s).ok())
        .unwrap_or_default()
}

pub fn write_projects_config(v: &Path, projects: &[ProjectMeta]) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(
        config_dir(v).join("projects.json"),
        serde_json::to_string_pretty(projects)?,
    )?;
    Ok(())
}

pub fn read_glossary(v: &Path) -> Vec<String> {
    std::fs::read_to_string(config_dir(v).join("glossary.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

pub fn write_glossary(v: &Path, text: &str) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(config_dir(v).join("glossary.txt"), text)?;
    Ok(())
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

pub fn read_project(v: &Path, slug: &str) -> Result<String> {
    read_entity(v, "project", slug)
}

pub fn read_entity(v: &Path, kind: &str, slug: &str) -> Result<String> {
    let slug = slugify(slug);
    let dir = dir_for_kind(v, kind).unwrap_or_else(|| projects_dir(v));
    Ok(std::fs::read_to_string(dir.join(format!("{slug}.md"))).unwrap_or_default())
}

/// Insert or replace this entity's section for `date`, keeping sections newest-first.
///
/// Within a date, each inbox `item_id` owns a subsection so two captures on the
/// same day both survive. Re-triaging the same item replaces only its subsection.
pub fn upsert_entity_day(
    v: &Path,
    kind: &str,
    slug: &str,
    name: &str,
    scope: &str,
    date: &str,
    item_id: &str,
    body: &str,
    date_fmt: &str,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let kind = if kind == "area" { "area" } else { "project" };
    let slug = slugify(slug);
    let dir = dir_for_kind(v, kind).unwrap_or_else(|| projects_dir(v));
    let path = dir.join(format!("{slug}.md"));
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    // Parse ## date sections. Inside each date, ### `item_id` subsections.
    // Content under a date with no item markers is kept as `_legacy`.
    // Date keys are always ISO; headings may be in the user's display format.
    let mut by_date: Vec<(String, Vec<(String, String)>)> = Vec::new();
    let mut cur_date: Option<String> = None;
    let mut cur_items: Vec<(String, String)> = Vec::new();
    let mut cur_item_id: Option<String> = None;
    let mut cur_body = String::new();

    let flush_item =
        |cur_item_id: &mut Option<String>, cur_body: &mut String, cur_items: &mut Vec<(String, String)>| {
            if let Some(id) = cur_item_id.take() {
                cur_items.push((id, cur_body.trim_end().to_string()));
                cur_body.clear();
            } else if !cur_body.trim().is_empty() {
                cur_items.push(("_legacy".into(), cur_body.trim_end().to_string()));
                cur_body.clear();
            }
        };

    for line in existing.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let candidate = rest.trim();
            if let Some(iso) = crate::datetime::parse_date_to_iso(candidate, date_fmt) {
                flush_item(&mut cur_item_id, &mut cur_body, &mut cur_items);
                if let Some(d) = cur_date.take() {
                    by_date.push((d, std::mem::take(&mut cur_items)));
                }
                cur_date = Some(iso);
                continue;
            }
        }
        if cur_date.is_none() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("### `") {
            if let Some((id, _)) = rest.split_once('`') {
                flush_item(&mut cur_item_id, &mut cur_body, &mut cur_items);
                cur_item_id = Some(id.to_string());
                continue;
            }
        }
        cur_body.push_str(line);
        cur_body.push('\n');
    }
    flush_item(&mut cur_item_id, &mut cur_body, &mut cur_items);
    if let Some(d) = cur_date.take() {
        by_date.push((d, cur_items));
    }

    let mut found = false;
    for (d, items) in &mut by_date {
        if d == date {
            items.retain(|(id, _)| id != item_id);
            items.push((item_id.to_string(), body.trim().to_string()));
            found = true;
            break;
        }
    }
    if !found {
        by_date.push((
            date.to_string(),
            vec![(item_id.to_string(), body.trim().to_string())],
        ));
    }
    by_date.sort_by(|a, b| b.0.cmp(&a.0));

    // Preserve existing Overview body; only Refresh summary / AI refresh rewrites it.
    let overview_body = extract_overview_body(&existing).unwrap_or_else(|| {
        "Standing view of recent activity. Full dated log is below.\n".into()
    });

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("slug: {slug}\n"));
    out.push_str(&format!("name: {name}\n"));
    out.push_str(&format!("type: {kind}\n"));
    out.push_str(&format!("scope: {scope}\n"));
    out.push_str("---\n\n");
    out.push_str(&format!("# {name}\n\n"));
    out.push_str("## Overview\n\n");
    out.push_str(overview_body.trim_end());
    out.push_str("\n\n");

    for (d, items) in by_date {
        let d_disp = crate::datetime::format_date(&d, date_fmt);
        out.push_str(&format!("## {d_disp}\n\n"));
        for (id, b) in items {
            if id == "_legacy" {
                if !b.is_empty() {
                    out.push_str(&b);
                    out.push_str("\n\n");
                }
            } else {
                out.push_str(&format!("### `{id}`\n\n"));
                if !b.is_empty() {
                    out.push_str(&b);
                    out.push_str("\n\n");
                }
            }
        }
    }
    std::fs::write(&path, out)?;
    Ok(())
}

fn extract_overview_body(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut start = None;
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "## Overview" {
            start = Some(i + 1);
            continue;
        }
        if start.is_some() && line.starts_with("## ") {
            end = i;
            break;
        }
    }
    let s = start?;
    let body = lines[s..end].join("\n").trim().to_string();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// Replace the ## Overview section body. Creates the section if missing.
pub fn replace_overview_section(content: &str, overview_body: &str) -> String {
    let body = overview_body.trim();
    let lines: Vec<&str> = content.lines().collect();
    let mut start = None;
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "## Overview" {
            start = Some(i);
            continue;
        }
        if start.is_some() && line.starts_with("## ") {
            end = i;
            break;
        }
    }
    let section = {
        let mut s = String::from("## Overview\n\n");
        s.push_str(body);
        if !body.ends_with('\n') {
            s.push('\n');
        }
        s.push('\n');
        s
    };
    if let Some(s) = start {
        let before = lines[..s].join("\n");
        let after = lines[end..].join("\n");
        let mut out = String::new();
        if !before.is_empty() {
            out.push_str(&before);
            out.push_str("\n\n");
        }
        out.push_str(&section);
        if !after.is_empty() {
            out.push_str(&after);
            if !after.ends_with('\n') {
                out.push('\n');
            }
        }
        return out;
    }
    // Insert after first # title line.
    let mut out = String::new();
    let mut inserted = false;
    for (i, line) in lines.iter().enumerate() {
        out.push_str(line);
        out.push('\n');
        if !inserted && line.starts_with("# ") {
            // Skip blank lines immediately after title, then insert.
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            out.push('\n');
            out.push_str(&section);
            for rest in lines.iter().skip(j) {
                out.push_str(rest);
                out.push('\n');
            }
            inserted = true;
            break;
        }
    }
    if !inserted {
        out.push_str(&section);
    }
    out
}

pub fn write_entity(v: &Path, kind: &str, slug: &str, content: &str) -> Result<()> {
    ensure_vault(v)?;
    let kind = if kind == "area" { "area" } else { "project" };
    let slug = slugify(slug);
    let dir = dir_for_kind(v, kind).unwrap_or_else(|| projects_dir(v));
    std::fs::write(dir.join(format!("{slug}.md")), content)?;
    Ok(())
}

pub fn write_personal(v: &Path, content: &str) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(personal_path(v), content)?;
    Ok(())
}

pub fn write_ideas(v: &Path, content: &str) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(ideas_path(v), content)?;
    Ok(())
}

pub fn write_tasks(v: &Path, content: &str) -> Result<()> {
    ensure_vault(v)?;
    std::fs::write(tasks_path(v), content)?;
    Ok(())
}

pub fn set_entity_overview(v: &Path, kind: &str, slug: &str, overview_body: &str) -> Result<()> {
    let existing = read_entity(v, kind, slug)?;
    let next = replace_overview_section(&existing, overview_body);
    write_entity(v, kind, slug, &next)
}

pub fn set_personal_overview(v: &Path, overview_body: &str) -> Result<()> {
    let existing = read_personal(v);
    let next = replace_overview_section(&existing, overview_body);
    write_personal(v, &next)
}

/// Create an empty project/area page and register it in projects.json.
pub fn create_entity(
    v: &Path,
    kind: &str,
    name: &str,
    scope: &str,
) -> Result<ProjectMeta> {
    ensure_vault(v)?;
    let kind = if kind == "area" { "area" } else { "project" };
    let scope = if scope == "personal" { "personal" } else { "work" };
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Name is required");
    }
    let slug = slugify(name);
    let dir = dir_for_kind(v, kind).unwrap_or_else(|| projects_dir(v));
    let path = dir.join(format!("{slug}.md"));
    if path.exists() {
        anyhow::bail!("A {kind} named '{slug}' already exists");
    }
    let body = format!(
        "---\nslug: {slug}\nname: {name}\ntype: {kind}\nscope: {scope}\n---\n\n\
         # {name}\n\n\
         ## Overview\n\n\
         - _(nothing filed yet)_\n\n"
    );
    std::fs::write(&path, body)?;

    let meta = ProjectMeta {
        slug: slug.clone(),
        name: name.to_string(),
        kind: kind.to_string(),
        scope: scope.to_string(),
        status: "active".into(),
        aliases: vec![],
        description: String::new(),
    };
    let mut known = read_projects_config(v);
    if !known.iter().any(|k| k.slug == slug) {
        known.push(meta.clone());
        write_projects_config(v, &known)?;
    }
    Ok(meta)
}

/// Delete a project/area file and drop it from projects.json.
pub fn delete_entity(v: &Path, kind: &str, slug: &str) -> Result<()> {
    ensure_vault(v)?;
    let kind = if kind == "area" { "area" } else { "project" };
    let slug = slugify(slug);
    let dir = dir_for_kind(v, kind).unwrap_or_else(|| projects_dir(v));
    let path = dir.join(format!("{slug}.md"));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let known: Vec<ProjectMeta> = read_projects_config(v)
        .into_iter()
        .filter(|p| p.slug != slug)
        .collect();
    write_projects_config(v, &known)?;
    Ok(())
}

/// Absolute path for a vault-relative path like `projects/daybook.md`.
pub fn vault_abs(v: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim().replace('\\', "/");
    if rel.contains("..") {
        anyhow::bail!("Invalid path");
    }
    Ok(v.join(rel))
}

#[derive(Debug, Serialize)]
pub struct ProjectEntry {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub scope: String,
    /// `active` | `paused` | `done`. Frontmatter, so it is hand-editable.
    pub status: String,
    pub last_date: String,
    pub day_count: usize,
}

fn list_entity_dir(dir: &Path, kind: &str, date_fmt: &str) -> Vec<ProjectEntry> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(slug) = name.strip_suffix(".md") else {
            continue;
        };
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        let display = text
            .lines()
            .find_map(|l| l.strip_prefix("name: "))
            .unwrap_or(slug)
            .trim()
            .to_string();
        let scope = text
            .lines()
            .find_map(|l| l.strip_prefix("scope: "))
            .unwrap_or("work")
            .trim()
            .to_string();
        let status = text
            .lines()
            .find_map(|l| l.strip_prefix("status: "))
            .unwrap_or("active")
            .trim()
            .to_string();
        // Date headings are written in the user's display format, so an ISO-only
        // parse finds nothing and every project looks untouched.
        let mut dates: Vec<String> = text
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .filter_map(|d| crate::datetime::parse_date_to_iso(d.trim(), date_fmt))
            .collect();
        dates.sort();
        dates.dedup();
        out.push(ProjectEntry {
            slug: slug.to_string(),
            name: display,
            kind: kind.to_string(),
            scope,
            status: if status.is_empty() { "active".into() } else { status },
            last_date: dates.last().cloned().unwrap_or_default(),
            day_count: dates.len(),
        });
    }
    out
}

pub fn list_projects(v: &Path, date_fmt: &str) -> Result<Vec<ProjectEntry>> {
    let mut out = list_entity_dir(&projects_dir(v), "project", date_fmt);
    out.extend(list_entity_dir(&areas_dir(v), "area", date_fmt));
    out.sort_by(|a, b| b.last_date.cmp(&a.last_date));
    Ok(out)
}

// ----------------------------------------------------------- ideas / tasks

/// Append a dated idea bullet. Ideas are maybe-someday; they don't own a file.
/// Append an idea. `link` is the owning `projects/slug` or `areas/slug` when the
/// idea belongs to something, so it stops floating free of its project.
pub fn append_idea(
    v: &Path,
    date: &str,
    time: &str,
    scope: &str,
    text: &str,
    date_fmt: &str,
    time_fmt: &str,
    entry_id: &str,
    link: Option<&str>,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = ideas_path(v);
    let mut existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Ideas\n\n".into());
    if !existing.ends_with('\n') {
        existing.push('\n');
    }

    let display_date = crate::datetime::format_date(date, date_fmt);
    let display_time = crate::datetime::format_time(time, time_fmt);
    let link_bit = link
        .filter(|l| !l.trim().is_empty())
        .map(|l| format!(" · [[{l}]]"))
        .unwrap_or_default();
    // The marker is invisible when rendered, and it is what stops a rebuild from
    // recovering a duplicate record for an idea that already has one.
    let bullet = format!(
        "- **{display_time}** <!-- e:{entry_id} --> ({scope}) {}{link_bit}\n",
        text.trim()
    );

    // Match an existing ## heading for the same calendar day (any display format).
    let mut insert_at: Option<usize> = None;
    for line in existing.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if crate::datetime::parse_date_to_iso(rest.trim(), date_fmt).as_deref() == Some(date) {
                if let Some(pos) = existing.find(line) {
                    let after = existing[pos..]
                        .find('\n')
                        .map(|i| pos + i + 1)
                        .unwrap_or(existing.len());
                    insert_at = Some(after);
                }
                break;
            }
        }
    }

    if let Some(after_heading) = insert_at {
        existing.insert_str(after_heading, &format!("\n{bullet}"));
    } else {
        existing.push_str(&format!("\n## {display_date}\n\n{bullet}"));
    }
    std::fs::write(&path, existing)?;
    Ok(())
}

/// Append an open task checkbox with capture date. `due` is optional YYYY-MM-DD.
/// Render one task line. Shared by append and rewrite so a task edited in the
/// app comes out byte-identical to one written at capture time.
pub fn format_task_line(
    done: bool,
    entry_id: &str,
    scope: &str,
    text: &str,
    date: &str,
    due: Option<&str>,
    link: Option<&str>,
    date_fmt: &str,
) -> String {
    let captured = crate::datetime::format_date(date, date_fmt);
    let due_bit = due
        .filter(|d| !d.trim().is_empty())
        .map(|d| format!(" · due {}", crate::datetime::format_date(d, date_fmt)))
        .unwrap_or_default();
    let link_bit = link
        .filter(|l| !l.trim().is_empty())
        .map(|l| format!(" · [[{l}]]"))
        .unwrap_or_default();
    let box_ = if done { "x" } else { " " };
    let scope_bit = if scope.trim().is_empty() {
        String::new()
    } else {
        format!("({scope}) ")
    };
    format!(
        "- [{box_}] <!-- e:{entry_id} --> {scope_bit}{} — captured {captured}{due_bit}{link_bit}",
        text.trim()
    )
}

/// Append a task. `entry_id` is carried in an HTML comment so the checkbox can
/// be matched back to its record — ticking the box in Obsidian has to count.
/// `link` is the owning `projects/slug` or `areas/slug`, if the task has one.
pub fn append_task(
    v: &Path,
    date: &str,
    scope: &str,
    text: &str,
    due: Option<&str>,
    date_fmt: &str,
    entry_id: &str,
    link: Option<&str>,
) -> Result<()> {
    valid_date(date)?;
    if let Some(d) = due {
        valid_date(d)?;
    }
    ensure_vault(v)?;
    let path = tasks_path(v);
    let mut existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Tasks\n\n".into());
    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&format_task_line(
        false, entry_id, scope, text, date, due, link, date_fmt,
    ));
    existing.push('\n');
    std::fs::write(&path, existing)?;
    Ok(())
}

/// Upsert one capture's section in the day note by splicing — freeform content
/// outside this item's glance line(s) and `## … · \`{id}\` ·` section is preserved.
pub fn upsert_day_item(
    v: &Path,
    date: &str,
    item_id: &str,
    time: &str,
    title: &str,
    summary_bullets: &[String],
    body: &str,
    date_fmt: &str,
    time_fmt: &str,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = days_dir(v).join(format!("{date}.md"));
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let title = title.trim();
    let title = if title.is_empty() { "Entry" } else { title };
    let title: String = title.chars().take(80).collect();
    let display_time = crate::datetime::format_time(time, time_fmt);
    let display_date = crate::datetime::format_date(date, date_fmt);

    let mut glance_lines = String::new();
    for b in summary_bullets {
        let t = b.trim();
        if !t.is_empty() {
            glance_lines.push_str(&format!("- <!-- item:{item_id} --> {t}\n"));
        }
    }

    let mut section = format!("## {display_time} · `{item_id}` · {title}\n\n");
    let body = body.trim();
    if !body.is_empty() {
        section.push_str(body);
        section.push_str("\n\n");
    }

    if existing.trim().is_empty() {
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("date: {date}\n"));
        out.push_str("type: daily\n");
        out.push_str("---\n\n");
        out.push_str(&format!("# {display_date}\n\n"));
        out.push_str("## At a glance\n\n");
        if glance_lines.is_empty() {
            out.push_str(&format!("- <!-- item:{item_id} --> {title}\n"));
        } else {
            out.push_str(&glance_lines);
        }
        out.push('\n');
        out.push_str(&section);
        out.push_str(&format!("---\n\nRaw: [[raw/{date}]]\n"));
        std::fs::write(&path, out)?;
        return Ok(());
    }

    let next = splice_day_item(&existing, item_id, &glance_lines, &section, date, &display_date);
    std::fs::write(&path, next)?;
    Ok(())
}

fn is_day_item_heading(line: &str, item_id: &str) -> bool {
    let Some(rest) = line.strip_prefix("## ") else {
        return false;
    };
    let Some((_, rest)) = rest.split_once(" · ") else {
        return false;
    };
    let Some((id_bit, _)) = rest.split_once(" · ") else {
        return false;
    };
    id_bit.trim().trim_matches('`') == item_id
}

fn is_raw_footer_sep(line: &str, next: Option<&str>) -> bool {
    line.trim() == "---" && next.is_some_and(|n| n.trim().starts_with("Raw:"))
}

/// Remove this item's glance lines + section, then re-insert updated ones.
fn splice_day_item(
    existing: &str,
    item_id: &str,
    glance_lines: &str,
    section: &str,
    iso_date: &str,
    display_date: &str,
) -> String {
    let glance_prefix = format!("- <!-- item:{item_id} -->");
    let lines: Vec<&str> = existing.lines().collect();
    let mut kept: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim_start().starts_with(&glance_prefix) {
            i += 1;
            continue;
        }
        if is_day_item_heading(line, item_id) {
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                let next = lines.get(i + 1).copied();
                if l.starts_with("## ") || is_raw_footer_sep(l, next) {
                    break;
                }
                i += 1;
            }
            continue;
        }
        kept.push(line.to_string());
        i += 1;
    }

    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }

    let has_title = kept.iter().any(|l| l.starts_with("# "));
    if !has_title {
        let mut insert_at = 0;
        if kept.first().map(|s| s.trim() == "---").unwrap_or(false) {
            insert_at = kept
                .iter()
                .skip(1)
                .position(|l| l.trim() == "---")
                .map(|p| p + 2)
                .unwrap_or(0);
        }
        kept.insert(insert_at, String::new());
        kept.insert(insert_at, format!("# {display_date}"));
    }

    let glance_idx = kept.iter().position(|l| l.trim() == "## At a glance");
    let glance_idx = if let Some(idx) = glance_idx {
        idx
    } else if let Some(title_idx) = kept.iter().position(|l| l.starts_with("# ")) {
        let mut at = title_idx + 1;
        while at < kept.len() && kept[at].trim().is_empty() {
            at += 1;
        }
        kept.insert(at, String::new());
        kept.insert(at, "## At a glance".into());
        at
    } else {
        kept.push("## At a glance".into());
        kept.len() - 1
    };

    let mut insert_glances_at = glance_idx + 1;
    if insert_glances_at < kept.len() && kept[insert_glances_at].trim().is_empty() {
        insert_glances_at += 1;
    }
    let glances_to_add: Vec<String> = glance_lines
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect();
    for (n, g) in glances_to_add.iter().enumerate() {
        kept.insert(insert_glances_at + n, g.clone());
    }

    let mut footer_at = kept.len();
    for (idx, line) in kept.iter().enumerate() {
        if line.trim().starts_with("Raw: [[raw/") {
            if idx > 0 && kept[idx - 1].trim() == "---" {
                footer_at = idx - 1;
            } else {
                footer_at = idx;
            }
            break;
        }
    }

    while footer_at > 0 && kept[footer_at - 1].trim().is_empty() {
        footer_at -= 1;
        kept.remove(footer_at);
    }
    let mut section_lines: Vec<String> = vec![String::new()];
    for l in section.lines() {
        section_lines.push(l.to_string());
    }
    for (n, l) in section_lines.iter().enumerate() {
        kept.insert(footer_at + n, l.clone());
    }

    let has_raw = kept.iter().any(|l| l.trim().starts_with("Raw: [[raw/"));
    if !has_raw {
        kept.push(String::new());
        kept.push("---".into());
        kept.push(String::new());
        kept.push(format!("Raw: [[raw/{iso_date}]]"));
    }

    let mut out = kept.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

// -------------------------------------------------------------- attachments

pub fn save_attachment(v: &Path, bytes: &[u8], ext: &str) -> Result<String> {
    ensure_vault(v)?;
    let ext: String = ext
        .trim_start_matches('.')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(5)
        .collect();
    let ext = if ext.is_empty() { "png".into() } else { ext };
    let stamp = Local::now().format("%Y-%m-%d-%H%M%S%3f").to_string();
    let name = format!("{stamp}.{ext}");
    std::fs::write(attachments_dir(v).join(&name), bytes)?;
    Ok(format!("attachments/{name}"))
}

pub fn is_image_ref(rel: &str) -> bool {
    let lower = rel.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Store a copy of a dropped file under its own name. Notion's "drop anything
/// in and it keeps a copy" only feels trustworthy if the name survives, so the
/// original is preserved and only de-duplicated on collision.
pub fn save_named_attachment(v: &Path, bytes: &[u8], filename: &str) -> Result<String> {
    ensure_vault(v)?;
    let raw = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let (stem, ext) = match raw.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, e),
        _ => (raw, ""),
    };
    let clean = |s: &str, max: usize| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim()
            .replace("  ", " ")
            .chars()
            .take(max)
            .collect()
    };
    let stem = clean(stem, 60);
    let stem = if stem.trim().is_empty() { "file".into() } else { stem };
    let ext: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_lowercase();

    let name_for = |suffix: &str| {
        if ext.is_empty() {
            format!("{stem}{suffix}")
        } else {
            format!("{stem}{suffix}.{ext}")
        }
    };

    let mut name = name_for("");
    if attachments_dir(v).join(&name).exists() {
        let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        name = name_for(&format!("-{stamp}"));
    }
    std::fs::write(attachments_dir(v).join(&name), bytes)?;
    Ok(format!("attachments/{name}"))
}

/// Markdown image refs like `![](attachments/foo.png)`.
pub fn extract_attachment_refs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while let Some(i) = text[search_from..].find("attachments/") {
        let start = search_from + i;
        let tail = &text[start..];
        let end = tail
            .find(|c: char| c == ')' || c == '"' || c == ' ' || c == '\n')
            .unwrap_or(tail.len());
        let path = tail[..end].trim();
        if !path.is_empty() && !out.iter().any(|p| p == path) {
            out.push(path.to_string());
        }
        search_from = start + path.len();
    }
    out
}

pub fn read_attachment_bytes(v: &Path, rel: &str) -> Result<Vec<u8>> {
    let rel = rel.trim().replace('\\', "/");
    if rel.contains("..") || !rel.starts_with("attachments/") {
        anyhow::bail!("Invalid attachment path: {rel}");
    }
    let path = v.join(&rel);
    std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
}

pub fn attachment_mime(rel: &str) -> String {
    let ext = rel.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "gif" => "image/gif".into(),
        "webp" => "image/webp".into(),
        _ => "image/png".into(),
    }
}

/// If the model dropped image markdown while rewriting, put the refs back.
pub fn ensure_attachment_markdown(original: &str, body: &str) -> String {
    let refs = extract_attachment_refs(original);
    if refs.is_empty() {
        return body.to_string();
    }
    let mut out = body.trim_end().to_string();
    for rel in refs {
        if out.contains(&rel) {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        // An `![]()` around a PDF renders as a broken image, so only images get
        // the embed; everything else becomes a plain link to the stored copy.
        if is_image_ref(&rel) {
            out.push_str(&format!("![]({rel})"));
        } else {
            let label = rel.rsplit('/').next().unwrap_or(&rel);
            out.push_str(&format!("[{label}]({rel})"));
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// data:image/...;base64,... — reliable in the webview without asset protocol.
pub fn attachment_data_url(v: &Path, rel: &str) -> Result<String> {
    let bytes = read_attachment_bytes(v, rel)?;
    let mime = attachment_mime(rel);
    let b64 = Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

// ----------------------------------------------------------- ideas / tasks read

pub fn read_ideas(v: &Path) -> String {
    ensure_vault(v).ok();
    std::fs::read_to_string(ideas_path(v)).unwrap_or_else(|_| "# Ideas\n\n".into())
}

pub fn read_tasks(v: &Path) -> String {
    ensure_vault(v).ok();
    std::fs::read_to_string(tasks_path(v)).unwrap_or_else(|_| "# Tasks\n\n".into())
}

pub fn read_personal(v: &Path) -> String {
    ensure_vault(v).ok();
    std::fs::read_to_string(personal_path(v)).unwrap_or_else(|_| "# Personal\n\n".into())
}

/// Remove every personal.md section belonging to a capture id (for re-triage).
pub fn clear_personal_item(v: &Path, item_id: &str, date_fmt: &str) -> Result<()> {
    ensure_vault(v)?;
    let path = personal_path(v);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.is_empty() || !existing.contains(&format!("`{item_id}`")) {
        return Ok(());
    }

    let mut by_date = parse_personal_by_date(&existing, date_fmt);
    for (_, items) in &mut by_date {
        items.retain(|(id, _, _, _, _)| id != item_id);
    }
    by_date.retain(|(_, items)| !items.is_empty());
    by_date.sort_by(|a, b| b.0.cmp(&a.0));
    let overview = extract_overview_body(&existing);
    std::fs::write(&path, render_personal_document(&by_date, date_fmt, overview.as_deref()))?;
    Ok(())
}

fn parse_personal_by_date(
    existing: &str,
    date_fmt: &str,
) -> Vec<(String, Vec<(String, String, String, String, String)>)> {
    let mut by_date: Vec<(String, Vec<(String, String, String, String, String)>)> = Vec::new();
    let mut cur_date: Option<String> = None;
    let mut cur_items: Vec<(String, String, String, String, String)> = Vec::new();
    let mut cur: Option<(String, String, String, String, String)> = None;

    let flush_item =
        |cur: &mut Option<(String, String, String, String, String)>,
         cur_items: &mut Vec<(String, String, String, String, String)>| {
            if let Some(mut item) = cur.take() {
                item.4 = item.4.trim_end().to_string();
                cur_items.push(item);
            }
        };

    for line in existing.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            let candidate = rest.trim();
            if let Some(iso) = crate::datetime::parse_date_to_iso(candidate, date_fmt) {
                flush_item(&mut cur, &mut cur_items);
                if let Some(d) = cur_date.take() {
                    by_date.push((d, std::mem::take(&mut cur_items)));
                }
                cur_date = Some(iso);
                continue;
            }
        }
        if cur_date.is_none() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("### ") {
            flush_item(&mut cur, &mut cur_items);
            let parts: Vec<&str> = rest.splitn(4, " · ").collect();
            if parts.len() >= 2 {
                cur = Some((
                    parts[1].trim().trim_matches('`').to_string(),
                    parts[0].trim().to_string(),
                    parts.get(2).unwrap_or(&"Entry").trim().to_string(),
                    parts.get(3).unwrap_or(&"note").trim().to_string(),
                    String::new(),
                ));
            }
            continue;
        }
        if let Some(ref mut c) = cur {
            c.4.push_str(line);
            c.4.push('\n');
        }
    }
    flush_item(&mut cur, &mut cur_items);
    if let Some(d) = cur_date.take() {
        by_date.push((d, cur_items));
    }
    by_date
}

fn render_personal_document(
    by_date: &[(String, Vec<(String, String, String, String, String)>)],
    date_fmt: &str,
    overview_body: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(
        "# Personal\n\n\
         Life notes and personal-scoped entries over time. Project work that happens to be\n\
         personal-scoped also appears here as a short pointer; the full log lives on the\n\
         project page.\n\n",
    );
    out.push_str("## Overview\n\n");
    let overview = overview_body.unwrap_or("Standing view of personal life threads. Dated log is below.");
    out.push_str(overview.trim_end());
    out.push_str("\n\n");
    for (d, items) in by_date {
        let d_disp = crate::datetime::format_date(d, date_fmt);
        out.push_str(&format!("## {d_disp}\n\n"));
        for (id, t, tit, dest, b) in items {
            out.push_str(&format!("### {t} · `{id}` · {tit} · {dest}\n\n"));
            if !b.is_empty() {
                out.push_str(b);
                out.push_str("\n\n");
            }
        }
    }
    out
}

/// Upsert one personal-scoped entry into personal.md (newest dates first).
/// `dest` is a short routing label, e.g. `note`, `[[projects/daybook|Daybook]]`, `tasks`.
pub fn upsert_personal_item(
    v: &Path,
    date: &str,
    item_id: &str,
    time: &str,
    title: &str,
    dest: &str,
    body: &str,
    date_fmt: &str,
    time_fmt: &str,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = personal_path(v);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut by_date = parse_personal_by_date(&existing, date_fmt);

    let display_time = crate::datetime::format_time(time, time_fmt);
    let entry = (
        item_id.to_string(),
        display_time,
        if title.trim().is_empty() {
            "Entry".into()
        } else {
            title.trim().chars().take(80).collect()
        },
        if dest.trim().is_empty() {
            "note".into()
        } else {
            dest.trim().to_string()
        },
        body.trim().to_string(),
    );

    let mut found = false;
    for (d, items) in &mut by_date {
        if d == date {
            items.push(entry.clone());
            items.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));
            found = true;
            break;
        }
    }
    if !found {
        by_date.push((date.to_string(), vec![entry]));
    }
    by_date.sort_by(|a, b| b.0.cmp(&a.0));
    let overview = extract_overview_body(&existing);
    std::fs::write(&path, render_personal_document(&by_date, date_fmt, overview.as_deref()))?;
    Ok(())
}

// ------------------------------------------------------------------- history

#[derive(Debug, Serialize)]
pub struct HistoryItem {
    pub id: String,
    pub date: String,
    pub time: String,
    pub preview: String,
    pub chars: usize,
    pub has_day_note: bool,
}

/// Chronological capture archive from raw/, newest first.
pub fn list_history(v: &Path, limit: usize) -> Result<Vec<HistoryItem>> {
    let mut items = Vec::new();
    let Ok(rd) = std::fs::read_dir(raw_dir(v)) else {
        return Ok(items);
    };
    let mut dates: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".md").map(|s| s.to_string())
        })
        .filter(|s| valid_date(s).is_ok())
        .collect();
    dates.sort();
    dates.reverse();

    for date in dates {
        let text = read_raw(v, &date).unwrap_or_default();
        let has_day_note = !read_note(v, &date).unwrap_or_default().trim().is_empty();
        let mut cur_time = String::new();
        let mut cur_id = String::new();
        let mut cur_body = String::new();

        let flush = |items: &mut Vec<HistoryItem>,
                     date: &str,
                     time: &str,
                     id: &str,
                     body: &str,
                     has_day_note: bool| {
            let body = body.trim();
            if body.is_empty() && time.is_empty() {
                return;
            }
            items.push(HistoryItem {
                id: id.to_string(),
                date: date.to_string(),
                time: time.to_string(),
                chars: body.chars().count(),
                preview: preview_of(body),
                has_day_note,
            });
        };

        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("## ") {
                flush(
                    &mut items,
                    &date,
                    &cur_time,
                    &cur_id,
                    &cur_body,
                    has_day_note,
                );
                cur_body.clear();
                cur_id.clear();
                // `15:30 · `id`` or just `15:30`
                if let Some((t, rest)) = rest.split_once(" · ") {
                    cur_time = t.trim().to_string();
                    cur_id = rest.trim().trim_matches('`').to_string();
                } else {
                    cur_time = rest.trim().to_string();
                }
                continue;
            }
            if line.starts_with("# ") {
                continue;
            }
            cur_body.push_str(line);
            cur_body.push('\n');
        }
        flush(
            &mut items,
            &date,
            &cur_time,
            &cur_id,
            &cur_body,
            has_day_note,
        );
        if items.len() >= limit {
            break;
        }
    }

    // Newest first: dates already newest-first; within a day sections were
    // flushed in file order (usually chronological), so reverse each day's
    // batch... easier: sort all by date desc, time desc.
    items.sort_by(|a, b| b.date.cmp(&a.date).then(b.time.cmp(&a.time)));
    items.truncate(limit);
    Ok(items)
}

pub fn read_history_item(v: &Path, date: &str, id: &str) -> Result<String> {
    valid_date(date)?;
    let text = read_raw(v, date)?;
    if id.trim().is_empty() {
        return Ok(text);
    }
    let needle = format!("`{id}`");
    let mut collecting = false;
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            if collecting {
                break;
            }
            collecting = line.contains(&needle);
            if collecting {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        if collecting {
            out.push_str(line);
            out.push('\n');
        }
    }
    if out.is_empty() {
        Ok(text)
    } else {
        Ok(out)
    }
}

/// Flip `- [ ]` ↔ `- [x]` on a 1-based line number in tasks.md.
pub fn toggle_task_line(v: &Path, line: usize) -> Result<String> {
    ensure_vault(v)?;
    let path = tasks_path(v);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Tasks\n\n".into());
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if line == 0 || line > lines.len() {
        anyhow::bail!("No task on line {line}");
    }
    let idx = line - 1;
    let l = &lines[idx];
    if l.contains("- [ ]") {
        lines[idx] = l.replacen("- [ ]", "- [x]", 1);
    } else if l.contains("- [x]") || l.contains("- [X]") {
        lines[idx] = l.replacen("- [x]", "- [ ]", 1).replacen("- [X]", "- [ ]", 1);
    } else {
        anyhow::bail!("Line {line} is not a task checkbox");
    }
    let out = if text.ends_with('\n') {
        format!("{}\n", lines.join("\n"))
    } else {
        lines.join("\n")
    };
    std::fs::write(&path, &out)?;
    Ok(out)
}

// ------------------------------------------------------------------ search

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub path: String,
    pub kind: String,
    pub date: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct Backlink {
    /// Vault-relative path of the file that links here, e.g. `days/2026-08-06.md`.
    pub path: String,
    pub kind: String,
    pub line: usize,
    pub text: String,
}

/// Normalize a wikilink target or a vault-relative path to a comparable key:
/// lowercase, forward slashes, no `.md`, no leading `./` or `/`.
fn link_key(s: &str) -> String {
    s.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches(".md")
        .to_lowercase()
}

/// Pull the target out of every `[[…]]` on a line, dropping `|alias` and `#anchor`.
fn wikilink_targets(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let rest = &line[i + 2..];
            if let Some(end) = rest.find("]]") {
                let inner = &rest[..end];
                let inner = inner.split('|').next().unwrap_or(inner);
                let inner = inner.split('#').next().unwrap_or(inner);
                if !inner.trim().is_empty() {
                    out.push(link_key(inner));
                }
                i += 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Every line in the vault that wikilinks to `target` (e.g. `projects/daybook`,
/// `days/2026-08-06`, `personal`). Obsidian's short form (`[[daybook]]`) counts too.
pub fn list_backlinks(v: &Path, target: &str, limit: usize) -> Result<Vec<Backlink>> {
    let want = link_key(target);
    if want.is_empty() {
        return Ok(vec![]);
    }
    let short = want.rsplit('/').next().unwrap_or(&want).to_string();

    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(v)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path
            .strip_prefix(v)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let kind = rel.split('/').next().unwrap_or("").to_string();
        // inbox/ is untriaged text, not part of the graph yet.
        if kind == "config" || kind == "inbox" || link_key(&rel) == want {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if !text.contains("[[") {
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            if wikilink_targets(line)
                .iter()
                .any(|t| *t == want || *t == short)
            {
                out.push(Backlink {
                    path: rel.clone(),
                    kind: kind.clone(),
                    line: i + 1,
                    text: line.trim().chars().take(300).collect(),
                });
                if out.len() >= limit {
                    return Ok(out);
                }
            }
        }
    }
    Ok(out)
}

pub fn search(v: &Path, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut hits = Vec::new();
    for entry in walkdir::WalkDir::new(v)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(v).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let kind = rel.split('/').next().unwrap_or("").to_string();
        if kind == "config" {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let date = if valid_date(&stem).is_ok() { stem } else { String::new() };

        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                hits.push(SearchHit {
                    path: rel.clone(),
                    kind: kind.clone(),
                    date: date.clone(),
                    line: i + 1,
                    text: line.trim().chars().take(300).collect(),
                });
                if hits.len() >= limit {
                    return Ok(hits);
                }
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAND_EDITED: &str = "\
---
date: 2026-08-06
type: daily
---

# 06/08/2026

## At a glance

- <!-- item:cap-1 --> Shipped the splice fix
- A bullet I typed myself

## Morning pages

Freeform prose I wrote by hand, outside any item section.

## 09:30 · `cap-1` · Shipped the splice fix

Body of the first capture.

---

Raw: [[raw/2026-08-06]]
";

    fn splice(existing: &str, id: &str, glance: &str, section: &str) -> String {
        splice_day_item(existing, id, glance, section, "2026-08-06", "06/08/2026")
    }

    #[test]
    fn new_item_preserves_hand_written_content() {
        let out = splice(
            HAND_EDITED,
            "cap-2",
            "- <!-- item:cap-2 --> Booked the dentist\n",
            "## 11:00 · `cap-2` · Booked the dentist\n\nBody of the second capture.\n\n",
        );

        // Everything the user wrote by hand survives.
        assert!(out.contains("- A bullet I typed myself"));
        assert!(out.contains("## Morning pages"));
        assert!(out.contains("Freeform prose I wrote by hand, outside any item section."));
        // The untouched item keeps its glance line and section.
        assert!(out.contains("- <!-- item:cap-1 --> Shipped the splice fix"));
        assert!(out.contains("## 09:30 · `cap-1` · Shipped the splice fix"));
        assert!(out.contains("Body of the first capture."));
        // The new item landed.
        assert!(out.contains("- <!-- item:cap-2 --> Booked the dentist"));
        assert!(out.contains("## 11:00 · `cap-2` · Booked the dentist"));
        // Footer stays single and last.
        assert_eq!(out.matches("Raw: [[raw/").count(), 1);
        assert!(out.trim_end().ends_with("Raw: [[raw/2026-08-06]]"));
    }

    #[test]
    fn reprocessing_an_item_replaces_it_without_duplicating() {
        let out = splice(
            HAND_EDITED,
            "cap-1",
            "- <!-- item:cap-1 --> Shipped the splice fix (revised)\n",
            "## 09:30 · `cap-1` · Shipped the splice fix\n\nRevised body.\n\n",
        );

        assert_eq!(out.matches("<!-- item:cap-1 -->").count(), 1);
        assert_eq!(out.matches("· `cap-1` ·").count(), 1);
        assert!(out.contains("Revised body."));
        assert!(!out.contains("Body of the first capture."));
        // Hand-written content is still untouched.
        assert!(out.contains("- A bullet I typed myself"));
        assert!(out.contains("## Morning pages"));
        assert!(out.contains("Freeform prose I wrote by hand, outside any item section."));
    }

    #[test]
    fn overview_body_is_extracted_for_preservation() {
        let page = "\
# Daybook

## Overview

My own standing summary.
Second line.

## 06/08/2026

- something dated
";
        assert_eq!(
            extract_overview_body(page).as_deref(),
            Some("My own standing summary.\nSecond line.")
        );
        assert_eq!(extract_overview_body("# No overview here\n"), None);
    }
}

/// Inbox items that have sat untouched for at least `min_idle_secs`.
///
/// Idleness comes from the file's modified time rather than the capture stamp,
/// so editing an item restarts its clock — auto-routing must never fire out
/// from under someone who is still typing.
pub fn list_inbox_idle(v: &Path, min_idle_secs: u64) -> Result<Vec<InboxItem>> {
    let now = std::time::SystemTime::now();
    let mut out = Vec::new();
    for item in list_inbox(v)? {
        let path = inbox_dir(v).join(format!("{}.md", item.id));
        let idle = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if idle >= min_idle_secs {
            out.push(item);
        }
    }
    Ok(out)
}
