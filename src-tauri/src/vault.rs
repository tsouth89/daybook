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

#[allow(dead_code)]
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn valid_date(date: &str) -> Result<NaiveDate> {
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

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

// --------------------------------------------------------------------- raw

/// Appends a timestamped block to a day's raw file. Never overwrites.
#[allow(dead_code)]
pub fn append_raw(v: &Path, date: &str, text: &str) -> Result<()> {
    append_raw_item(v, date, None, text)
}

pub fn append_raw_item(v: &Path, date: &str, item_id: Option<&str>, text: &str) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = raw_dir(v).join(format!("{date}.md"));

    let mut out = String::new();
    if !path.exists() {
        out.push_str(&format!("# {date} (raw)\n\n"));
    }
    let stamp = Local::now().format("%H:%M").to_string();
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
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub description: String,
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
            if valid_date(candidate).is_ok() {
                flush_item(&mut cur_item_id, &mut cur_body, &mut cur_items);
                if let Some(d) = cur_date.take() {
                    by_date.push((d, std::mem::take(&mut cur_items)));
                }
                cur_date = Some(candidate.to_string());
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

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("slug: {slug}\n"));
    out.push_str(&format!("name: {name}\n"));
    out.push_str(&format!("type: {kind}\n"));
    out.push_str(&format!("scope: {scope}\n"));
    out.push_str("---\n\n");
    out.push_str(&format!("# {name}\n\n"));
    out.push_str("## Overview\n\n");
    out.push_str("Standing view of recent activity. Full dated log is below.\n\n");
    let mut overview_lines = 0usize;
    for (d, items) in &by_date {
        for (_id, b) in items {
            let title = overview_title_from_body(b);
            if title.is_empty() {
                continue;
            }
            out.push_str(&format!("- **{d}**: {title}\n"));
            overview_lines += 1;
            if overview_lines >= 8 {
                break;
            }
        }
        if overview_lines >= 8 {
            break;
        }
    }
    if overview_lines == 0 {
        out.push_str("- _(nothing filed yet)_\n");
    }
    out.push('\n');

    for (d, items) in by_date {
        out.push_str(&format!("## {d}\n\n"));
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

fn overview_title_from_body(body: &str) -> String {
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Some(rest) = t.strip_prefix("**").and_then(|s| s.strip_suffix("**")) {
            return rest.trim().chars().take(100).collect();
        }
        if t.starts_with("**") && t.ends_with("**") {
            return t.trim_matches('*').trim().chars().take(100).collect();
        }
        if !t.starts_with('#') && !t.starts_with("![") && !t.starts_with("[[") {
            return t.trim_start_matches(['*', '-', '#', ' ']).chars().take(100).collect();
        }
    }
    String::new()
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

#[derive(Debug, Serialize)]
pub struct ProjectEntry {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub scope: String,
    pub last_date: String,
    pub day_count: usize,
}

fn list_entity_dir(dir: &Path, kind: &str) -> Vec<ProjectEntry> {
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
        let dates: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("## "))
            .map(str::trim)
            .filter(|d| valid_date(d).is_ok())
            .collect();
        out.push(ProjectEntry {
            slug: slug.to_string(),
            name: display,
            kind: kind.to_string(),
            scope,
            last_date: dates.first().unwrap_or(&"").to_string(),
            day_count: dates.len(),
        });
    }
    out
}

pub fn list_projects(v: &Path) -> Result<Vec<ProjectEntry>> {
    let mut out = list_entity_dir(&projects_dir(v), "project");
    out.extend(list_entity_dir(&areas_dir(v), "area"));
    out.sort_by(|a, b| b.last_date.cmp(&a.last_date));
    Ok(out)
}

// ----------------------------------------------------------- ideas / tasks

/// Append a dated idea bullet. Ideas are maybe-someday; they don't own a file.
pub fn append_idea(v: &Path, date: &str, time: &str, scope: &str, text: &str) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = ideas_path(v);
    let mut existing = std::fs::read_to_string(&path).unwrap_or_else(|_| "# Ideas\n\n".into());
    if !existing.ends_with('\n') {
        existing.push('\n');
    }

    let heading = format!("## {date}");
    let bullet = format!("- **{time}** ({scope}) {}\n", text.trim());

    if let Some(pos) = existing.find(&heading) {
        // Insert after the heading line.
        let after_heading = existing[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(existing.len());
        existing.insert_str(after_heading, &format!("\n{bullet}"));
    } else {
        existing.push_str(&format!("\n{heading}\n\n{bullet}"));
    }
    std::fs::write(&path, existing)?;
    Ok(())
}

/// Append an open task checkbox with capture date. `due` is optional YYYY-MM-DD.
pub fn append_task(
    v: &Path,
    date: &str,
    scope: &str,
    text: &str,
    due: Option<&str>,
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
    let due_bit = due.map(|d| format!(" · due {d}")).unwrap_or_default();
    existing.push_str(&format!(
        "- [ ] ({scope}) {} — captured {date}{due_bit}\n",
        text.trim()
    ));
    std::fs::write(&path, existing)?;
    Ok(())
}

/// Upsert one capture's section in the day note. Day notes are a view over
/// routed entries; each inbox item owns a section keyed by its id.
pub fn upsert_day_item(
    v: &Path,
    date: &str,
    item_id: &str,
    time: &str,
    title: &str,
    summary_bullets: &[String],
    body: &str,
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = days_dir(v).join(format!("{date}.md"));
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let mut glances: Vec<(String, String)> = Vec::new();
    let mut sections: Vec<(String, String, String, String)> = Vec::new();
    let mut cur: Option<(String, String, String, String)> = None;

    for line in existing.lines() {
        if let Some(rest) = line.strip_prefix("- <!-- item:") {
            if let Some((id, rest)) = rest.split_once(" --> ") {
                glances.push((id.trim().to_string(), rest.to_string()));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some((t, rest)) = rest.split_once(" · ") {
                let section_time = t.trim().to_string();
                if let Some((id_bit, section_title)) = rest.split_once(" · ") {
                    let id = id_bit.trim().trim_matches('`').to_string();
                    if let Some(prev) = cur.take() {
                        sections.push(prev);
                    }
                    cur = Some((
                        id,
                        section_time,
                        section_title.trim().to_string(),
                        String::new(),
                    ));
                    continue;
                }
            }
        }
        if let Some(ref mut c) = cur {
            c.3.push_str(line);
            c.3.push('\n');
        }
    }
    if let Some(prev) = cur.take() {
        sections.push(prev);
    }

    glances.retain(|(id, _)| id != item_id);
    for b in summary_bullets {
        let t = b.trim();
        if !t.is_empty() {
            glances.push((item_id.to_string(), t.to_string()));
        }
    }

    sections.retain(|(id, _, _, _)| id != item_id);
    let title = title.trim();
    let title = if title.is_empty() { "Entry" } else { title };
    sections.push((
        item_id.to_string(),
        time.to_string(),
        title.chars().take(80).collect(),
        body.trim().to_string(),
    ));
    sections.sort_by(|a, b| a.1.cmp(&b.1));

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("date: {date}\n"));
    out.push_str("type: daily\n");
    out.push_str("---\n\n");
    out.push_str(&format!("# {date}\n\n"));
    if !glances.is_empty() {
        out.push_str("## At a glance\n\n");
        for (id, text) in &glances {
            out.push_str(&format!("- <!-- item:{id} --> {text}\n"));
        }
        out.push('\n');
    }
    for (id, t, section_title, b) in &sections {
        out.push_str(&format!("## {t} · `{id}` · {section_title}\n\n"));
        if !b.is_empty() {
            out.push_str(b);
            out.push_str("\n\n");
        }
    }
    out.push_str(&format!("---\n\nRaw: [[raw/{date}]]\n"));
    std::fs::write(&path, out)?;
    Ok(())
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
        out.push_str(&format!("![]({rel})"));
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
pub fn clear_personal_item(v: &Path, item_id: &str) -> Result<()> {
    ensure_vault(v)?;
    let path = personal_path(v);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.is_empty() || !existing.contains(&format!("`{item_id}`")) {
        return Ok(());
    }

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
            if valid_date(candidate).is_ok() {
                flush_item(&mut cur, &mut cur_items);
                if let Some(d) = cur_date.take() {
                    by_date.push((d, std::mem::take(&mut cur_items)));
                }
                cur_date = Some(candidate.to_string());
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

    for (_, items) in &mut by_date {
        items.retain(|(id, _, _, _, _)| id != item_id);
    }
    by_date.retain(|(_, items)| !items.is_empty());
    by_date.sort_by(|a, b| b.0.cmp(&a.0));
    std::fs::write(&path, render_personal_document(&by_date))?;
    Ok(())
}

fn render_personal_document(
    by_date: &[(String, Vec<(String, String, String, String, String)>)],
) -> String {
    let mut out = String::new();
    out.push_str(
        "# Personal\n\n\
         Life notes and personal-scoped entries over time. Project work that happens to be\n\
         personal-scoped also appears here as a short pointer; the full log lives on the\n\
         project page.\n\n",
    );
    out.push_str("## Overview\n\n");
    out.push_str("Standing view of personal life threads. Dated log is below.\n\n");
    let mut overview_lines = 0usize;
    for (d, items) in by_date {
        for (_id, _t, tit, dest, _b) in items {
            out.push_str(&format!("- **{d}**: {tit} · {dest}\n"));
            overview_lines += 1;
            if overview_lines >= 8 {
                break;
            }
        }
        if overview_lines >= 8 {
            break;
        }
    }
    if overview_lines == 0 {
        out.push_str("- _(nothing filed yet)_\n");
    }
    out.push('\n');
    for (d, items) in by_date {
        out.push_str(&format!("## {d}\n\n"));
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
) -> Result<()> {
    valid_date(date)?;
    ensure_vault(v)?;
    let path = personal_path(v);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    // (id, time, title, dest, body)
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
            if valid_date(candidate).is_ok() {
                flush_item(&mut cur, &mut cur_items);
                if let Some(d) = cur_date.take() {
                    by_date.push((d, std::mem::take(&mut cur_items)));
                }
                cur_date = Some(candidate.to_string());
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

    let entry = (
        item_id.to_string(),
        time.to_string(),
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
    std::fs::write(&path, render_personal_document(&by_date))?;
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
