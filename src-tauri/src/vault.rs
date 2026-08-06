use anyhow::{Context, Result};
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
             `ideas.md`, `tasks.md`, and `days/`.\n\n\
             Only `inbox/` and `raw/` hold original text. Everything else is a build artifact and\n\
             can be deleted and regenerated. This folder is a valid Obsidian vault — open it\n\
             directly for graph view, backlinks, or hand-written notes in the right place.\n",
        )?;
    }

    for (name, heading) in [("ideas.md", "# Ideas\n\n"), ("tasks.md", "# Tasks\n\n")] {
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
