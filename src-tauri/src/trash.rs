//! Undo for destructive actions.
//!
//! Deleting used to be final. That was tolerable while every write came from a
//! person clicking a button; it stops being tolerable the moment triage can act
//! on instructions and an external assistant can drive the vault over MCP. A
//! mistake made on your behalf has to be one click to reverse.
//!
//! Trash lives under `config/`, which search, backlinks, and the entry index
//! already skip — so nothing in here can leak back into a view while it waits
//! to be restored or purged.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Payload {
    /// One routed entry. Tasks also had a line in `tasks.md`.
    Entry {
        record: crate::entries::EntryRecord,
    },
    /// A project or area page, plus its `projects.json` row.
    Entity {
        entity_kind: String,
        slug: String,
        markdown: String,
        meta: Option<crate::vault::ProjectMeta>,
    },
    /// A capture discarded before it was ever routed.
    Inbox { id: String, contents: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    pub label: String,
    pub deleted_at: String,
    pub payload: Payload,
}

pub fn trash_dir(v: &Path) -> PathBuf {
    crate::vault::config_dir(v).join("trash")
}

fn slot(v: &Path, id: &str) -> PathBuf {
    trash_dir(v).join(format!("{id}.json"))
}

fn new_id() -> String {
    // Counter as well as clock: two deletions inside one clock tick are easy to
    // do by holding a key down, and a collision would silently drop one.
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    format!(
        "{}-{:04}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        N.fetch_add(1, Ordering::Relaxed) % 10000
    )
}

pub fn put(v: &Path, label: &str, payload: Payload) -> Result<String> {
    std::fs::create_dir_all(trash_dir(v))?;
    let item = TrashItem {
        id: new_id(),
        label: label.chars().take(120).collect(),
        deleted_at: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        payload,
    };
    std::fs::write(slot(v, &item.id), serde_json::to_string_pretty(&item)?)?;
    Ok(item.id)
}

/// Newest first. A malformed file is skipped rather than fatal, same rule as
/// the entry index.
pub fn list(v: &Path) -> Vec<TrashItem> {
    let Ok(rd) = std::fs::read_dir(trash_dir(v)) else {
        return vec![];
    };
    let mut out: Vec<TrashItem> = rd
        .flatten()
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| serde_json::from_str::<TrashItem>(&t).ok())
        .collect();
    out.sort_by(|a, b| b.id.cmp(&a.id));
    out
}

fn take(v: &Path, id: &str) -> Result<TrashItem> {
    let path = slot(v, id);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("nothing in the trash with id {id}"))?;
    let item: TrashItem = serde_json::from_str(&text)?;
    std::fs::remove_file(&path)?;
    Ok(item)
}

/// Put something back. Restoring is best-effort by design: if the thing has
/// been recreated in the meantime, putting the old copy back on top would
/// destroy the newer one, so that case is reported instead.
pub fn restore(v: &Path, id: &str, date_fmt: &str) -> Result<String> {
    let item = take(v, id)?;
    match item.payload {
        Payload::Entry { record } => {
            let mut all = crate::entries::load(v);
            if all.iter().any(|r| r.id == record.id) {
                // Put the trash record back so nothing is lost by refusing.
                put(v, &item.label, Payload::Entry { record })?;
                anyhow::bail!("That entry exists again; delete it first if you want the old one.");
            }
            if record.kind == "task" {
                crate::vault::append_task(
                    v,
                    &record.date,
                    &record.scope,
                    &record.title,
                    record.due.as_deref(),
                    date_fmt,
                    &record.id,
                    crate::entries::entity_link(v, &record.slug).as_deref(),
                )?;
            }
            let label = record.title.clone();
            all.push(record);
            all.sort_by(|a, b| (&a.date, &a.time, &a.id).cmp(&(&b.date, &b.time, &b.id)));
            crate::entries::write_all_records(v, &all)?;
            Ok(label)
        }
        Payload::Entity {
            entity_kind,
            slug,
            markdown,
            meta,
        } => {
            let dir = crate::vault::dir_for_kind(v, &entity_kind)
                .unwrap_or_else(|| crate::vault::projects_dir(v));
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{slug}.md"));
            if path.exists() {
                put(
                    v,
                    &item.label,
                    Payload::Entity { entity_kind, slug: slug.clone(), markdown, meta },
                )?;
                anyhow::bail!("A page called {slug} exists again; rename it first.");
            }
            std::fs::write(&path, markdown)?;
            if let Some(meta) = meta {
                let mut known = crate::vault::read_projects_config(v);
                if !known.iter().any(|m| m.slug == meta.slug) {
                    known.push(meta);
                    crate::vault::write_projects_config(v, &known)?;
                }
            }
            Ok(slug)
        }
        Payload::Inbox { id: item_id, contents } => {
            let path = crate::vault::inbox_dir(v).join(format!("{item_id}.md"));
            std::fs::create_dir_all(crate::vault::inbox_dir(v))?;
            if path.exists() {
                put(v, &item.label, Payload::Inbox { id: item_id, contents })?;
                anyhow::bail!("That capture is back in the inbox already.");
            }
            std::fs::write(&path, contents)?;
            Ok(item_id)
        }
    }
}

pub fn purge(v: &Path, id: &str) -> Result<()> {
    take(v, id)?;
    Ok(())
}

pub fn empty(v: &Path) -> Result<usize> {
    let items = list(v);
    let n = items.len();
    for i in items {
        let _ = std::fs::remove_file(slot(v, &i.id));
    }
    Ok(n)
}
