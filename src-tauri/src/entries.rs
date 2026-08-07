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
    /// `triage` for records written when the capture was processed, `recovered`
    /// for ones parsed back out of existing markdown. Rebuilds replace every
    /// `recovered` record and never touch a `triage` one.
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "triage".into()
}

pub const SOURCE_TRIAGE: &str = "triage";
pub const SOURCE_RECOVERED: &str = "recovered";

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
    /// Case-insensitive match across the text an entry carries.
    pub text: Option<String>,
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

pub fn write_all_records(v: &Path, records: &[EntryRecord]) -> Result<()> {
    write_all(v, records)
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

pub fn marker_id_of(line: &str) -> Option<String> {
    marker_id(line)
}

/// Tick or untick a task by record id. The markdown is what changes — the index
/// holds no done state of its own, so this stays true whether the box is
/// flipped here or by hand in Obsidian.
pub fn set_task_done(v: &Path, entry_id: &str, done: bool) -> Result<()> {
    let path = crate::vault::tasks_path(v);
    let text = std::fs::read_to_string(&path)?;
    let mut changed = false;
    let out: Vec<String> = text
        .lines()
        .map(|line| {
            if changed || marker_id(line).as_deref() != Some(entry_id) {
                return line.to_string();
            }
            let t = line.trim_start();
            let rest = match t
                .strip_prefix("- [ ]")
                .or_else(|| t.strip_prefix("- [x]"))
                .or_else(|| t.strip_prefix("- [X]"))
            {
                Some(r) => r,
                None => return line.to_string(),
            };
            changed = true;
            let indent = &line[..line.len() - t.len()];
            format!("{indent}- [{}]{rest}", if done { "x" } else { " " })
        })
        .collect();

    if !changed {
        anyhow::bail!("No task in tasks.md with id {entry_id}");
    }
    let mut joined = out.join("\n");
    if !joined.ends_with('\n') {
        joined.push('\n');
    }
    std::fs::write(&path, joined)?;
    Ok(())
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

/// Every string an entry carries is searchable, including the structured lists —
/// "what did I decide about auth" should find a `decisions` bullet.
fn matches_text(r: &EntryRecord, needle: &str) -> bool {
    let direct = [&r.title, &r.body, &r.name, &r.slug]
        .iter()
        .any(|s| s.to_lowercase().contains(needle));
    if direct {
        return true;
    }
    [&r.accomplished, &r.decisions, &r.open]
        .iter()
        .any(|list| list.iter().any(|s| s.to_lowercase().contains(needle)))
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
            if let Some(needle) = &q.text {
                let needle = needle.trim().to_lowercase();
                if !needle.is_empty() && !matches_text(r, &needle) {
                    return false;
                }
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
            source: SOURCE_TRIAGE.into(),
        }
    }

    fn tmp() -> PathBuf {
        // A counter, not a timestamp: Windows clock granularity is ~15ms, so
        // parallel tests starting in the same tick would share a directory.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!(
            "daybook-test-{}-{}-{}",
            std::process::id(),
            module_path!().replace("::", "-"),
            n
        ));
        let _ = std::fs::remove_dir_all(&p);
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
    fn ticking_a_task_writes_through_to_the_markdown() {
        let v = tmp();
        crate::vault::append_task(
            &v, "2026-08-06", "work", "Write tests", None, "DD/MM/YYYY", "cap-1-e0", None,
        )
        .unwrap();

        set_task_done(&v, "cap-1-e0", true).unwrap();
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("- [x] <!-- e:cap-1-e0 -->"), "got: {text}");
        assert_eq!(task_state(&v).get("cap-1-e0"), Some(&true));

        set_task_done(&v, "cap-1-e0", false).unwrap();
        assert_eq!(task_state(&v).get("cap-1-e0"), Some(&false));
        // The rest of the line is untouched by the flip.
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("Write tests"));

        assert!(set_task_done(&v, "no-such-id", true).is_err());
    }

    #[test]
    fn correcting_a_task_updates_both_halves() {
        let v = tmp();
        let created = create_entry(
            &v,
            EntryRecord {
                kind: "task".into(),
                scope: "work".into(),
                title: "Write tests".into(),
                date: "2026-08-06".into(),
                ..rec("", "", "task", "", "2026-08-06")
            },
            "DD/MM/YYYY",
        )
        .unwrap();

        // Triage guessed the wrong project and no due date; fix both.
        let mut fixed = created.clone();
        fixed.slug = "daybook".into();
        fixed.due = Some("2026-08-12".into());
        fixed.title = "Write tests for routing".into();
        update_entry(&v, &fixed, "DD/MM/YYYY").unwrap();

        // The index changed...
        let stored = load(&v).into_iter().find(|r| r.id == created.id).unwrap();
        assert_eq!(stored.slug, "daybook");
        assert_eq!(stored.due.as_deref(), Some("2026-08-12"));

        // ...and so did the markdown, which is the half you read in Obsidian.
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("Write tests for routing"), "got: {text}");
        assert!(text.contains("[[projects/daybook]]"), "got: {text}");
        assert!(text.contains("due 12/08/2026"), "got: {text}");
        assert_eq!(text.matches("<!-- e:").count(), 1, "no duplicate line");
    }

    #[test]
    fn editing_a_task_keeps_it_ticked() {
        let v = tmp();
        let created = create_entry(
            &v,
            EntryRecord {
                kind: "task".into(),
                title: "Ship it".into(),
                ..rec("", "", "task", "", "2026-08-06")
            },
            "DD/MM/YYYY",
        )
        .unwrap();
        set_task_done(&v, &created.id, true).unwrap();

        let mut edited = created.clone();
        edited.title = "Ship it properly".into();
        update_entry(&v, &edited, "DD/MM/YYYY").unwrap();

        assert_eq!(task_state(&v).get(&created.id), Some(&true), "still done");
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("Ship it properly"));
    }

    #[test]
    fn deleting_a_task_removes_its_line_too() {
        let v = tmp();
        let created = create_entry(
            &v,
            EntryRecord {
                kind: "task".into(),
                title: "Wrong entirely".into(),
                ..rec("", "", "task", "", "2026-08-06")
            },
            "DD/MM/YYYY",
        )
        .unwrap();
        delete_entry(&v, &created.id).unwrap();

        assert!(load(&v).is_empty());
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(!text.contains("Wrong entirely"), "got: {text}");
        assert!(delete_entry(&v, &created.id).is_err(), "second delete reports");
    }

    #[test]
    fn deleting_is_undoable() {
        let v = tmp();
        let created = create_entry(
            &v,
            EntryRecord {
                kind: "task".into(),
                scope: "work".into(),
                slug: "daybook".into(),
                title: "Do not lose me".into(),
                ..rec("", "", "task", "", "2026-08-06")
            },
            "DD/MM/YYYY",
        )
        .unwrap();

        delete_entry(&v, &created.id).unwrap();
        assert!(load(&v).is_empty());
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(!text.contains("Do not lose me"));

        let bin = crate::trash::list(&v);
        assert_eq!(bin.len(), 1);
        assert_eq!(bin[0].label, "Do not lose me");

        crate::trash::restore(&v, &bin[0].id, "DD/MM/YYYY").unwrap();
        let back = load(&v);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].title, "Do not lose me");
        // A task's markdown line comes back too, not just the record.
        let text = std::fs::read_to_string(crate::vault::tasks_path(&v)).unwrap();
        assert!(text.contains("Do not lose me"), "got: {text}");
        assert!(crate::trash::list(&v).is_empty(), "restored items leave the bin");
    }

    #[test]
    fn restore_refuses_to_clobber_a_replacement() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let r = rec("cap-1-e0", "cap-1", "note", "", "2026-08-06");
        replace_item(&v, "cap-1", &[r.clone()]).unwrap();
        delete_entry(&v, "cap-1-e0").unwrap();

        // The same id exists again by the time you press undo.
        let mut newer = r.clone();
        newer.title = "The newer one".into();
        replace_item(&v, "cap-1", &[newer]).unwrap();

        let bin = crate::trash::list(&v);
        assert!(crate::trash::restore(&v, &bin[0].id, "DD/MM/YYYY").is_err());
        // Refusing must not throw the trashed copy away.
        assert_eq!(crate::trash::list(&v).len(), 1);
        assert_eq!(load(&v)[0].title, "The newer one");
    }

    #[test]
    fn an_open_loop_can_be_closed() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let mut r = rec("cap-1-e0", "cap-1", "project", "daybook", "2026-08-06");
        r.open = vec!["Pick an index format".into(), "Decide on backfill".into()];
        replace_item(&v, "cap-1", &[r]).unwrap();

        resolve_open(&v, "cap-1-e0", "Pick an index format").unwrap();
        let stored = load(&v).into_iter().next().unwrap();
        assert_eq!(stored.open, vec!["Decide on backfill"]);
        assert!(resolve_open(&v, "cap-1-e0", "not a real loop").is_err());
    }

    #[test]
    fn update_cannot_rewrite_provenance() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let mut original = rec("cap-1-e0", "cap-1", "note", "", "2026-08-06");
        original.source = SOURCE_RECOVERED.into();
        replace_item(&v, "cap-1", &[original.clone()]).unwrap();

        let mut sneaky = original.clone();
        sneaky.item_id = "someone-else".into();
        sneaky.source = SOURCE_TRIAGE.into();
        sneaky.title = "New title".into();
        update_entry(&v, &sneaky, "DD/MM/YYYY").unwrap();

        let stored = load(&v).into_iter().next().unwrap();
        assert_eq!(stored.title, "New title", "editable fields still apply");
        assert_eq!(stored.item_id, "cap-1", "identity is not the caller's to set");
        assert_eq!(stored.source, SOURCE_RECOVERED, "provenance is preserved");
    }

    #[test]
    fn retrieval_ranks_relevant_entries_and_drops_the_rest() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let mut auth = rec("cap-1-e0", "cap-1", "project", "daybook", "2026-08-01");
        auth.decisions = vec!["Used session cookies for auth, simpler than JWT".into()];
        let mut other = rec("cap-2-e0", "cap-2", "project", "bmx-site", "2026-08-02");
        other.body = "Painted the frame".into();
        replace_item(&v, "cap-1", &[auth]).unwrap();
        replace_item(&v, "cap-2", &[other]).unwrap();

        let hits = retrieve(&v, "what did I decide about auth?", 5);
        assert_eq!(hits.len(), 1, "unrelated entries are not padding");
        assert_eq!(hits[0].id, "cap-1-e0");

        // Stopwords alone must not drag everything in.
        assert!(retrieve(&v, "what about the and for", 5).is_empty());

        let ctx = as_context(&hits);
        assert!(ctx.contains("decided: Used session cookies"));
        assert!(ctx.contains("[2026-08-01]"));
    }

    #[test]
    fn text_search_reaches_into_the_structured_lists() {
        let v = tmp();
        crate::vault::ensure_vault(&v).unwrap();
        let mut r = rec("cap-1-e0", "cap-1", "project", "daybook", "2026-08-06");
        r.decisions = vec!["Kept raw append-only so rebuilds stay safe".into()];
        r.open = vec!["Pick an index format".into()];
        r.body = "Wired up the router.".into();
        replace_item(&v, "cap-1", &[r]).unwrap();

        let hit = |t: &str| {
            query(
                &v,
                &EntryQuery {
                    text: Some(t.into()),
                    ..Default::default()
                },
            )
            .len()
        };
        assert_eq!(hit("append-only"), 1, "decisions are searchable");
        assert_eq!(hit("index format"), 1, "open loops are searchable");
        assert_eq!(hit("ROUTER"), 1, "case-insensitive over body");
        assert_eq!(hit("daybook"), 1, "slug is searchable");
        assert_eq!(hit("nothing here"), 0);
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

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "was", "were", "what", "when", "where", "did", "does", "with", "that",
    "this", "have", "has", "had", "about", "from", "into", "any", "all", "you", "your", "are",
    "not", "but", "how", "why", "who", "which", "there", "their", "then", "than", "been", "being",
    "some", "just", "get", "got", "can", "will", "would", "should", "could", "still", "over",
];

fn terms(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2 && !STOPWORDS.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Pick the entries most likely to answer a question. Deliberately dumb: term
/// overlap plus a nudge for recency. It works because the entries being scored
/// are already split and routed, not whole files of mixed content.
pub fn retrieve(v: &Path, question: &str, limit: usize) -> Vec<EntryRecord> {
    let terms = terms(question);
    let all = load(v);
    if all.is_empty() {
        return vec![];
    }
    let newest = all.iter().map(|r| r.date.as_str()).max().unwrap_or("").to_string();

    let mut scored: Vec<(usize, &EntryRecord)> = all
        .iter()
        .map(|r| {
            let hay = format!(
                "{} {} {} {} {} {} {}",
                r.title,
                r.body,
                r.name,
                r.slug,
                r.accomplished.join(" "),
                r.decisions.join(" "),
                r.open.join(" ")
            )
            .to_lowercase();
            let mut score: usize = terms.iter().filter(|t| hay.contains(t.as_str())).count() * 10;
            if score > 0 {
                // Break ties toward what is still open and what happened lately.
                if !r.open.is_empty() {
                    score += 3;
                }
                if r.date == newest {
                    score += 2;
                }
            }
            (score, r)
        })
        .filter(|(s, _)| *s > 0)
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.date.cmp(&a.1.date)));
    scored.into_iter().take(limit).map(|(_, r)| r.clone()).collect()
}

/// Compact, model-facing rendering of the entries retrieved for a question.
pub fn as_context(records: &[EntryRecord]) -> String {
    let mut out = String::new();
    for r in records {
        out.push_str(&format!("[{}] {}", r.date, r.kind));
        if !r.name.is_empty() || !r.slug.is_empty() {
            out.push_str(&format!(
                " · {}",
                if r.name.is_empty() { &r.slug } else { &r.name }
            ));
        }
        if !r.title.is_empty() {
            out.push_str(&format!(" · {}", r.title));
        }
        out.push('\n');
        if !r.accomplished.is_empty() {
            out.push_str(&format!("  accomplished: {}\n", r.accomplished.join("; ")));
        }
        if !r.decisions.is_empty() {
            out.push_str(&format!("  decided: {}\n", r.decisions.join("; ")));
        }
        if !r.open.is_empty() {
            out.push_str(&format!("  open: {}\n", r.open.join("; ")));
        }
        if let Some(d) = &r.due {
            out.push_str(&format!("  due: {d}\n"));
        }
        let body = r.body.trim();
        if !body.is_empty() {
            let short: String = body.chars().take(600).collect();
            out.push_str(&format!("  {}\n", short.replace('\n', " ")));
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// Editing. Triage's first guess has to be correctable, or the index becomes a
// set of claims you cannot argue with.
// ---------------------------------------------------------------------------

/// Where a task's owning project link points, given the vault's known entities.
pub fn entity_link(v: &Path, slug: &str) -> Option<String> {
    link_for(v, slug)
}

fn link_for(v: &Path, slug: &str) -> Option<String> {
    let slug = slug.trim();
    if slug.is_empty() {
        return None;
    }
    let dir = match crate::vault::read_projects_config(v)
        .into_iter()
        .find(|m| m.slug == slug)
        .map(|m| m.kind)
        .as_deref()
    {
        Some("area") => "areas",
        _ => "projects",
    };
    Some(format!("{dir}/{slug}"))
}

/// Rewrite the `tasks.md` line for a record, preserving its done state. The
/// line is a pure rendering of the record, so it is regenerated rather than
/// patched field by field.
fn rewrite_task_line(v: &Path, r: &EntryRecord, date_fmt: &str) -> Result<()> {
    let path = crate::vault::tasks_path(v);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let mut found = false;
    let out: Vec<String> = text
        .lines()
        .map(|line| {
            if found || marker_id(line).as_deref() != Some(r.id.as_str()) {
                return line.to_string();
            }
            let t = line.trim_start();
            if !(t.starts_with("- [") ) {
                return line.to_string();
            }
            found = true;
            let done = t.starts_with("- [x]") || t.starts_with("- [X]");
            let indent = &line[..line.len() - t.len()];
            format!(
                "{indent}{}",
                crate::vault::format_task_line(
                    done,
                    &r.id,
                    &r.scope,
                    &r.title,
                    &r.date,
                    r.due.as_deref(),
                    link_for(v, &r.slug).as_deref(),
                    date_fmt,
                )
            )
        })
        .collect();
    if !found {
        return Ok(());
    }
    let mut joined = out.join("\n");
    if !joined.ends_with('\n') {
        joined.push('\n');
    }
    std::fs::write(&path, joined)?;
    Ok(())
}

fn remove_task_line(v: &Path, entry_id: &str) -> Result<()> {
    let path = crate::vault::tasks_path(v);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let kept: Vec<&str> = text
        .lines()
        .filter(|l| marker_id(l).as_deref() != Some(entry_id))
        .collect();
    let mut joined = kept.join("\n");
    if !joined.ends_with('\n') {
        joined.push('\n');
    }
    std::fs::write(&path, joined)?;
    Ok(())
}

/// Replace one record wholesale. The caller sends the whole edited record back
/// rather than a patch — this is a single-user local app, and read-modify-write
/// on one file avoids inventing a merge story for no benefit.
pub fn update_entry(v: &Path, updated: &EntryRecord, date_fmt: &str) -> Result<()> {
    let mut all = load(v);
    let Some(slot) = all.iter_mut().find(|r| r.id == updated.id) else {
        anyhow::bail!("No entry with id {}", updated.id);
    };
    // Identity and provenance are not the caller's to change.
    let mut next = updated.clone();
    next.item_id = slot.item_id.clone();
    next.source = slot.source.clone();
    if next.kind.trim().is_empty() {
        next.kind = slot.kind.clone();
    }
    if next.date.trim().is_empty() {
        next.date = slot.date.clone();
    }
    if let Some(d) = &next.due {
        if d.trim().is_empty() {
            next.due = None;
        } else {
            crate::vault::valid_date(d)?;
        }
    }
    crate::vault::valid_date(&next.date)?;
    *slot = next.clone();
    all.sort_by(|a, b| (&a.date, &a.time, &a.id).cmp(&(&b.date, &b.time, &b.id)));
    write_all(v, &all)?;

    if next.kind == "task" {
        rewrite_task_line(v, &next, date_fmt)?;
    }
    Ok(())
}

/// Drop one entry. For tasks the markdown line goes too, since it is a pure
/// rendering; other kinds leave their prose alone because you may have edited it.
pub fn delete_entry(v: &Path, entry_id: &str) -> Result<()> {
    let mut all = load(v);
    let before = all.len();
    let doomed = all.iter().find(|r| r.id == entry_id).cloned();
    let was_task = doomed.as_ref().map(|r| r.kind == "task").unwrap_or(false);
    all.retain(|r| r.id != entry_id);
    if all.len() == before {
        anyhow::bail!("No entry with id {entry_id}");
    }
    if let Some(record) = doomed {
        let label = if record.title.trim().is_empty() {
            record.kind.clone()
        } else {
            record.title.clone()
        };
        crate::trash::put(v, &label, crate::trash::Payload::Entry { record })?;
    }
    write_all(v, &all)?;
    if was_task {
        remove_task_line(v, entry_id)?;
    }
    Ok(())
}

/// Close one open loop. These are the things Home nags about, so being able to
/// say "that's settled" without editing prose is the whole point of listing them.
pub fn resolve_open(v: &Path, entry_id: &str, line: &str) -> Result<()> {
    let mut all = load(v);
    let Some(slot) = all.iter_mut().find(|r| r.id == entry_id) else {
        anyhow::bail!("No entry with id {entry_id}");
    };
    let before = slot.open.len();
    slot.open.retain(|o| o.trim() != line.trim());
    if slot.open.len() == before {
        anyhow::bail!("That loop is not on entry {entry_id}");
    }
    write_all(v, &all)
}

/// Create an entry by hand, with no capture behind it. Notion lets you just add
/// a row; requiring dictation for every task would be a strange thing to insist on.
pub fn create_entry(v: &Path, mut r: EntryRecord, date_fmt: &str) -> Result<EntryRecord> {
    crate::vault::ensure_vault(v)?;
    if r.date.trim().is_empty() {
        r.date = crate::vault::today();
    }
    crate::vault::valid_date(&r.date)?;
    if let Some(d) = &r.due {
        if d.trim().is_empty() {
            r.due = None;
        } else {
            crate::vault::valid_date(d)?;
        }
    }
    if r.title.trim().is_empty() {
        anyhow::bail!("Give it a title.");
    }
    if r.kind.trim().is_empty() {
        r.kind = "note".into();
    }
    if r.scope.trim().is_empty() {
        r.scope = "personal".into();
    }

    let stamp = chrono::Local::now().format("%H%M%S").to_string();
    r.item_id = format!("manual-{}-{}", r.date, stamp);
    r.id = format!("{}-e0", r.item_id);
    r.source = SOURCE_TRIAGE.into();
    if r.time.trim().is_empty() {
        r.time = chrono::Local::now().format("%H:%M").to_string();
    }

    if r.kind == "task" {
        crate::vault::append_task(
            v,
            &r.date,
            &r.scope,
            &r.title,
            r.due.as_deref(),
            date_fmt,
            &r.id,
            link_for(v, &r.slug).as_deref(),
        )?;
    }

    let mut all = load(v);
    all.push(r.clone());
    all.sort_by(|a, b| (&a.date, &a.time, &a.id).cmp(&(&b.date, &b.time, &b.id)));
    write_all(v, &all)?;
    Ok(r)
}
