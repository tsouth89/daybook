//! The item layer: routed entries with their triage properties kept intact.
//!
//! Markdown pages stay the human-readable, hand-editable half of the vault.
//! This is the queryable half — what makes "show me everything open on
//! Daybook" a filter instead of a grep.
//!
//! It is a build artifact, not a source of truth. Every record here was derived
//! from a capture in `raw/`, which the AI never edits, so this file can be
//! deleted and rebuilt. It lives under `config/` because that directory is
//! already excluded from search, backlinks, and the Obsidian-facing vault.
//!
//! Properties split by who owns them:
//! - **Capture-time facts** (scope, slug, due, accomplished/decisions/open)
//!   come from triage and live here.
//! - **Mutable state** (a task being done) lives in the markdown, because
//!   ticking a box in Obsidian has to count. It is parsed back at query time.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryRecord {
    /// Stable id, `{item_id}-e{n}`. Survives reprocessing of the same capture.
    pub id: String,
    /// The capture this was split out of.
    pub item_id: String,
    pub date: String,
    pub time: String,
    /// `personal` | `work`
    pub scope: String,
    /// `project` | `area` | `idea` | `task` | `note`
    pub kind: String,
    /// Owning project/area slug. Empty when the entry belongs to nothing.
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub accomplished: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub open: Vec<String>,
    #[serde(default)]
    pub due: Option<String>,
}

/// A record plus the mutable state parsed back out of the markdown.
#[derive(Debug, Clone, Serialize)]
pub struct EntryView {
    #[serde(flatten)]
    pub record: EntryRecord,
    /// Tasks only: whether the checkbox is ticked in `tasks.md`.
    pub done: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct EntryQuery {
    pub scope: Option<String>,
    pub kind: Option<String>,
    pub slug: Option<String>,
    pub date: Option<String>,
    /// Inclusive ISO lower bound.
    pub since: Option<String>,
    /// Only entries carrying unresolved `open` loops.
    pub open_only: bool,
    /// Tasks only: drop the ones already ticked off.
    pub undone_only: bool,
    pub limit: Option<usize>,
}

pub fn entries_path(v: &Path) -> PathBuf {
    crate::vault::config_dir(v).join("entries.jsonl")
}

pub fn entry_id(item_id: &str, n: usize) -> String {
    format!("{item_id}-e{n}")
}

/// Load every record. Malformed lines are skipped rather than fatal: this file
/// is regenerable, and one bad line must never take out the whole index.
pub fn load(v: &Path) -> Vec<EntryRecord> {
    let Ok(text) = std::fs::read_to_string(entries_path(v)) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<EntryRecord>(l).ok())
        .collect()
}

fn write_all(v: &Path, records: &[EntryRecord]) -> Result<()> {
    crate::vault::ensure_vault(v)?;
    let mut out = String::new();
    for r in records {
        out.push_str(&serde_json::to_string(r)?);
        out.push('\n');
    }
    std::fs::write(entries_path(v), out)?;
    Ok(())
}

/// Replace every record belonging to one capture. Reprocessing the same item
/// updates in place instead of accumulating duplicates.
pub fn replace_item(v: &Path, item_id: &str, records: &[EntryRecord]) -> Result<()> {
    let mut all = load(v);
    all.retain(|r| r.item_id != item_id);
    all.extend(records.iter().cloned());
    all.sort_by(|a, b| (&a.date, &a.time, &a.id).cmp(&(&b.date, &b.time, &b.id)));
    write_all(v, &all)
}

/// Drop a capture's records, for when an item is removed rather than reprocessed.
pub fn remove_item(v: &Path, item_id: &str) -> Result<()> {
    let mut all = load(v);
    let before = all.len();
    all.retain(|r| r.item_id != item_id);
    if all.len() == before {
        return Ok(());
    }
    write_all(v, &all)
}

/// Which task entries are ticked off, read back from `tasks.md`.
pub fn task_state(v: &Path) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    let Ok(text) = std::fs::read_to_string(crate::vault::tasks_path(v)) else {
        return out;
    };
    for line in text.lines() {
        let t = line.trim_start();
        let done = if t.starts_with("- [x]") || t.starts_with("- [X]") {
            true
        } else if t.starts_with("- [ ]") {
            false
        } else {
            continue;
        };
        if let Some(id) = marker_id(t) {
            out.insert(id, done);
        }
    }
    out
}

/// Pull `{id}` out of an `<!-- e:{id} -->` marker.
fn marker_id(line: &str) -> Option<String> {
    let start = line.find("<!-- e:")? + "<!-- e:".len();
    let rest = &line[start..];
    let end = rest.find("-->")?;
    let id = rest[..end].trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

pub fn query(v: &Path, q: &EntryQuery) -> Vec<EntryView> {
    let done_map = task_state(v);
    let mut out: Vec<EntryView> = load(v)
        .into_iter()
        .filter(|r| {
            if let Some(s) = &q.scope {
                if !s.is_empty() && &r.scope != s {
                    return false;
                }
            }
            if let Some(k) = &q.kind {
                if !k.is_empty() && &r.kind != k {
                    return false;
                }
            }
            if let Some(s) = &q.slug {
                if !s.is_empty() && &r.slug != s {
                    return false;
                }
            }
            if let Some(d) = &q.date {
                if !d.is_empty() && &r.date != d {
                    return false;
                }
            }
            if let Some(s) = &q.since {
                if !s.is_empty() && r.date.as_str() < s.as_str() {
                    return false;
                }
            }
            if q.open_only && r.open.is_empty() {
                return false;
            }
            true
        })
        .map(|r| {
            let done = done_map.get(&r.id).copied().unwrap_or(false);
            EntryView { record: r, done }
        })
        .filter(|e| !(q.undone_only && e.record.kind == "task" && e.done))
        .collect();

    // Newest first — every view of this wants recent activity at the top.
    out.sort_by(|a, b| {
        (&b.record.date, &b.record.time).cmp(&(&a.record.date, &a.record.time))
    });
    if let Some(n) = q.limit {
        out.truncate(n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, item: &str, kind: &str, slug: &str, date: &str) -> EntryRecord {
        EntryRecord {
            id: id.into(),
            item_id: item.into(),
            date: date.into(),
            time: "09:30".into(),
            scope: "work".into(),
            kind: kind.into(),
            slug: slug.into(),
            name: String::new(),
            title: "t".into(),
            body: String::new(),
            accomplished: vec![],
            decisions: vec![],
            open: vec![],
            due: None,
        }
    }

    fn tmp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "daybook-entries-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn reprocessing_replaces_instead_of_duplicating() {
        let v = tmp();
        replace_item(&v, "cap-1", &[rec("cap-1-e0", "cap-1", "task", "", "2026-08-06")]).unwrap();
        replace_item(
            &v,
            "cap-1",
            &[
                rec("cap-1-e0", "cap-1", "task", "daybook", "2026-08-06"),
                rec("cap-1-e1", "cap-1", "note", "", "2026-08-06"),
            ],
        )
        .unwrap();
        let all = load(&v);
        assert_eq!(all.len(), 2);
        assert_eq!(all.iter().filter(|r| r.id == "cap-1-e0").count(), 1);
        assert_eq!(all[0].slug, "daybook");
    }

    #[test]
    fn other_items_are_untouched_by_replace_and_remove() {
        let v = tmp();
        replace_item(&v, "cap-1", &[rec("cap-1-e0", "cap-1", "task", "", "2026-08-06")]).unwrap();
        replace_item(&v, "cap-2", &[rec("cap-2-e0", "cap-2", "note", "", "2026-08-07")]).unwrap();
        replace_item(&v, "cap-1", &[rec("cap-1-e0", "cap-1", "idea", "", "2026-08-06")]).unwrap();
        assert_eq!(load(&v).len(), 2);
        remove_item(&v, "cap-1").unwrap();
        let all = load(&v);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].item_id, "cap-2");
    }

    #[test]
    fn a_malformed_line_does_not_kill_the_index() {
        let v = tmp();
        replace_item(&v, "cap-1", &[rec("cap-1-e0", "cap-1", "task", "", "2026-08-06")]).unwrap();
        let p = entries_path(&v);
        let mut text = std::fs::read_to_string(&p).unwrap();
        text.push_str("{ this is not json\n");
        std::fs::write(&p, text).unwrap();
        assert_eq!(load(&v).len(), 1);
    }

    #[test]
    fn done_state_is_read_back_from_the_markdown() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        std::fs::write(
            crate::vault::tasks_path(&v),
            "# Tasks\n\n\
             - [x] <!-- e:cap-1-e0 --> (work) Write tests — captured 06/08/2026\n\
             - [ ] <!-- e:cap-1-e1 --> (work) Ship it — captured 06/08/2026\n\
             - [ ] a hand-written task with no marker\n",
        )
        .unwrap();
        let state = task_state(&v);
        assert_eq!(state.get("cap-1-e0"), Some(&true));
        assert_eq!(state.get("cap-1-e1"), Some(&false));
        assert_eq!(state.len(), 2);
    }

    /// The contract between the two halves: a task the vault writes as markdown
    /// must be matchable back to the record that produced it.
    #[test]
    fn a_task_written_as_markdown_is_matched_back_to_its_record() {
        let v = tmp();
        crate::vault::append_task(
            &v,
            "2026-08-06",
            "work",
            "Write tests for the routing",
            Some("2026-08-12"),
            "DD/MM/YYYY",
            "cap-1-e0",
            Some("projects/daybook"),
        )
        .unwrap();

        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        // The project link is visible markdown, so it works in Obsidian too.
        assert!(text.contains("[[projects/daybook]]"), "got: {text}");
        assert!(text.contains("due 12/08/2026"), "got: {text}");

        let state = task_state(&v);
        assert_eq!(state.get("cap-1-e0"), Some(&false));

        // Ticking the box by hand is what flips the state.
        std::fs::write(
            crate::vault::tasks_path(&v),
            text.replace("- [ ]", "- [x]"),
        )
        .unwrap();
        assert_eq!(task_state(&v).get("cap-1-e0"), Some(&true));
    }

    #[test]
    fn query_filters_by_project_and_hides_finished_tasks() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let mut open_one = rec("cap-1-e2", "cap-1", "project", "daybook", "2026-08-05");
        open_one.open = vec!["Decide on the index format".into()];
        replace_item(
            &v,
            "cap-1",
            &[
                rec("cap-1-e0", "cap-1", "task", "daybook", "2026-08-06"),
                rec("cap-1-e1", "cap-1", "task", "bmx-site", "2026-08-06"),
                open_one,
            ],
        )
        .unwrap();
        std::fs::write(
            crate::vault::tasks_path(&v),
            "- [x] <!-- e:cap-1-e0 --> (work) done one\n",
        )
        .unwrap();

        let by_project = query(
            &v,
            &EntryQuery {
                slug: Some("daybook".into()),
                ..Default::default()
            },
        );
        assert_eq!(by_project.len(), 2);

        let outstanding = query(
            &v,
            &EntryQuery {
                kind: Some("task".into()),
                undone_only: true,
                ..Default::default()
            },
        );
        assert_eq!(outstanding.len(), 1);
        assert_eq!(outstanding[0].record.slug, "bmx-site");

        let loops = query(
            &v,
            &EntryQuery {
                open_only: true,
                ..Default::default()
            },
        );
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].record.open, vec!["Decide on the index format"]);
    }
}
