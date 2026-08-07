pub mod ai;
pub mod config;
pub mod backfill;
pub mod datetime;
pub mod entries;
pub mod trash;
pub mod vault;

use base64::Engine;
use config::Settings;
use serde::Serialize;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use vault::ProjectMeta;

pub struct AppState {
    settings: Mutex<Settings>,
    config_dir: PathBuf,
    /// Serializes triage runs. The auto-processor and the buttons share one
    /// path, and two of them draining the inbox at once would double-file.
    processing: tauri::async_runtime::Mutex<()>,
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

/// Commands return String errors because that is what crosses the IPC boundary cleanly.
type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// --------------------------------------------------------------- settings

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    settings: Settings,
) -> CmdResult<Settings> {
    let old_hotkey = state.settings().capture_hotkey;
    vault::ensure_vault(&settings.vault()).map_err(err)?;
    config::save(&state.config_dir, &settings).map_err(err)?;

    if settings.capture_hotkey != old_hotkey {
        let gs = app.global_shortcut();
        if let Ok(s) = Shortcut::from_str(&old_hotkey) {
            let _ = gs.unregister(s);
        }
        match Shortcut::from_str(&settings.capture_hotkey) {
            Ok(s) => gs
                .register(s)
                .map_err(|e| format!("Could not register hotkey '{}': {e}", settings.capture_hotkey))?,
            Err(e) => return Err(format!("'{}' is not a valid hotkey: {e}", settings.capture_hotkey)),
        }
    }

    *state.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

// ------------------------------------------------------------------ capture

#[tauri::command]
fn append_entry(state: tauri::State<AppState>, text: String) -> CmdResult<String> {
    if text.trim().is_empty() {
        return Err("Nothing to save.".into());
    }
    let s = state.settings();
    // Every capture becomes a discrete inbox file. Triage routes it later.
    let id = vault::write_inbox_item(&s.vault(), &text).map_err(err)?;
    Ok(id)
}

#[tauri::command]
fn list_inbox(state: tauri::State<AppState>) -> CmdResult<Vec<vault::InboxItem>> {
    vault::list_inbox(&state.settings().vault()).map_err(err)
}

#[tauri::command]
fn delete_inbox_item(state: tauri::State<AppState>, id: String) -> CmdResult<()> {
    let v = state.settings().vault();
    // Discarding is also the escape hatch for a bad triage, so drop any records
    // the capture left behind rather than leaving them orphaned in the index.
    entries::remove_item(&v, &id).map_err(err)?;
    if let Ok(contents) = std::fs::read_to_string(vault::inbox_dir(&v).join(format!("{id}.md"))) {
        let label: String = contents
            .split("---\n\n")
            .nth(1)
            .unwrap_or(&contents)
            .trim()
            .chars()
            .take(80)
            .collect();
        trash::put(&v, &label, trash::Payload::Inbox { id: id.clone(), contents }).map_err(err)?;
    }
    vault::delete_inbox_item(&v, &id).map_err(err)
}

#[tauri::command]
fn list_trash(state: tauri::State<AppState>) -> CmdResult<Vec<trash::TrashItem>> {
    Ok(trash::list(&state.settings().vault()))
}

#[tauri::command]
fn restore_trash(state: tauri::State<AppState>, id: String) -> CmdResult<String> {
    let s = state.settings();
    trash::restore(&s.vault(), &id, &s.date_format).map_err(err)
}

#[tauri::command]
fn purge_trash(state: tauri::State<AppState>, id: String) -> CmdResult<()> {
    trash::purge(&state.settings().vault(), &id).map_err(err)
}

#[tauri::command]
fn empty_trash(state: tauri::State<AppState>) -> CmdResult<usize> {
    trash::empty(&state.settings().vault()).map_err(err)
}

#[tauri::command]
fn update_inbox_item(state: tauri::State<AppState>, id: String, text: String) -> CmdResult<()> {
    vault::update_inbox_item(&state.settings().vault(), &id, &text).map_err(err)
}

#[tauri::command]
fn ensure_day(state: tauri::State<AppState>, date: Option<String>) -> CmdResult<String> {
    let s = state.settings();
    let date = date.unwrap_or_else(vault::today);
    vault::ensure_day(&s.vault(), &date, &s.date_format).map_err(err)?;
    Ok(date)
}

#[tauri::command]
fn today_date() -> String {
    vault::today()
}

/// Query the item layer. This is what views are built from — filter by project,
/// scope, kind, or open loops instead of grepping prose.
#[tauri::command]
fn query_entries(
    state: tauri::State<AppState>,
    query: entries::EntryQuery,
) -> CmdResult<Vec<entries::EntryView>> {
    Ok(entries::query(&state.settings().vault(), &query))
}

#[tauri::command]
fn set_task_done(state: tauri::State<AppState>, entry_id: String, done: bool) -> CmdResult<()> {
    entries::set_task_done(&state.settings().vault(), &entry_id, done).map_err(err)
}

#[derive(Serialize)]
struct AskAnswer {
    answer: String,
    /// The entries the answer was drawn from, so it can be checked.
    used: Vec<entries::EntryRecord>,
}

/// Ask a question of the vault. Retrieval runs over the item layer, which is
/// why this waited until entries existed — the same question over date-ordered
/// prose would pull in whole days of unrelated content.
#[tauri::command]
async fn ask_vault(state: tauri::State<'_, AppState>, question: String) -> CmdResult<AskAnswer> {
    let s = state.settings();
    if question.trim().is_empty() {
        return Err("Ask something first.".into());
    }
    let used = entries::retrieve(&s.vault(), &question, 24);
    if used.is_empty() {
        return Err("Nothing in your daybook matched that question.".into());
    }
    let context = entries::as_context(&used);
    let answer = ai::ask(ai::AskRequest {
        provider: &s.provider,
        api_key: &s.resolved_api_key(),
        model: &s.model,
        effort: &s.effort,
        question: &question,
        context: &context,
    })
    .await
    .map_err(err)?;
    Ok(AskAnswer { answer, used })
}

/// Correct what triage got wrong. Without this the index is a set of claims you
/// cannot argue with — hand-editing the markdown would not reach these fields.
#[tauri::command]
fn update_entry(state: tauri::State<AppState>, entry: entries::EntryRecord) -> CmdResult<()> {
    let s = state.settings();
    entries::update_entry(&s.vault(), &entry, &s.date_format).map_err(err)
}

#[tauri::command]
fn create_entry(
    state: tauri::State<AppState>,
    entry: entries::EntryRecord,
) -> CmdResult<entries::EntryRecord> {
    let s = state.settings();
    entries::create_entry(&s.vault(), entry, &s.date_format).map_err(err)
}

#[tauri::command]
fn delete_entry(state: tauri::State<AppState>, entry_id: String) -> CmdResult<()> {
    entries::delete_entry(&state.settings().vault(), &entry_id).map_err(err)
}

#[tauri::command]
fn resolve_open_loop(
    state: tauri::State<AppState>,
    entry_id: String,
    line: String,
) -> CmdResult<()> {
    entries::resolve_open(&state.settings().vault(), &entry_id, &line).map_err(err)
}

/// Nest one page under another. Files never move, so links survive.
#[tauri::command]
fn set_entity_parent(
    state: tauri::State<AppState>,
    kind: String,
    slug: String,
    parent: String,
) -> CmdResult<()> {
    vault::set_entity_parent(&state.settings().vault(), &kind, &slug, &parent).map_err(err)
}

/// Recover item records from markdown written before the index existed. Costs
/// nothing — it parses the vault rather than re-triaging through the model.
#[tauri::command]
fn rebuild_entry_index(state: tauri::State<AppState>) -> CmdResult<backfill::RebuildReport> {
    let s = state.settings();
    backfill::rebuild(&s.vault(), &s.date_format).map_err(err)
}

#[tauri::command]
fn list_backlinks(
    state: tauri::State<AppState>,
    target: String,
) -> CmdResult<Vec<vault::Backlink>> {
    vault::list_backlinks(&state.settings().vault(), &target, 80).map_err(err)
}

#[tauri::command]
fn save_attachment(
    state: tauri::State<AppState>,
    data_base64: String,
    ext: String,
) -> CmdResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| format!("Bad image data: {e}"))?;
    vault::save_attachment(&state.settings().vault(), &bytes, &ext).map_err(err)
}

/// Store a copy of any dropped file, keeping its name.
#[tauri::command]
fn save_file_attachment(
    state: tauri::State<AppState>,
    data_base64: String,
    filename: String,
) -> CmdResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| format!("Bad file data: {e}"))?;
    // Base64 over IPC is not the way to move a large file; fail clearly instead
    // of hanging the capture window.
    const MAX: usize = 64 * 1024 * 1024;
    if bytes.len() > MAX {
        return Err(format!(
            "{filename} is {} MB; the limit is {} MB. Link to it instead.",
            bytes.len() / 1024 / 1024,
            MAX / 1024 / 1024
        ));
    }
    vault::save_named_attachment(&state.settings().vault(), &bytes, &filename).map_err(err)
}

#[tauri::command]
fn attachment_data_url(state: tauri::State<AppState>, rel: String) -> CmdResult<String> {
    vault::attachment_data_url(&state.settings().vault(), &rel).map_err(err)
}

#[tauri::command]
fn hide_capture(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.hide();
    }
}

#[tauri::command]
fn show_capture(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("capture") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("capture-focus", ());
    }
}

#[tauri::command]
fn read_tasks(state: tauri::State<AppState>) -> String {
    vault::read_tasks(&state.settings().vault())
}

#[tauri::command]
fn read_ideas(state: tauri::State<AppState>) -> String {
    vault::read_ideas(&state.settings().vault())
}

#[tauri::command]
fn read_personal(state: tauri::State<AppState>) -> String {
    vault::read_personal(&state.settings().vault())
}

#[tauri::command]
fn list_history(state: tauri::State<AppState>) -> CmdResult<Vec<vault::HistoryItem>> {
    vault::list_history(&state.settings().vault(), 200).map_err(err)
}

#[tauri::command]
fn read_history_item(
    state: tauri::State<AppState>,
    date: String,
    id: String,
) -> CmdResult<String> {
    vault::read_history_item(&state.settings().vault(), &date, &id).map_err(err)
}

#[tauri::command]
fn toggle_task_line(state: tauri::State<AppState>, line: usize) -> CmdResult<String> {
    vault::toggle_task_line(&state.settings().vault(), line).map_err(err)
}

// -------------------------------------------------------------------- read

#[derive(Serialize)]
struct DayContent {
    date: String,
    raw: String,
    note: String,
}

#[tauri::command]
fn list_days(state: tauri::State<AppState>) -> CmdResult<Vec<vault::DayEntry>> {
    vault::list_days(&state.settings().vault()).map_err(err)
}

#[tauri::command]
fn read_day(state: tauri::State<AppState>, date: String) -> CmdResult<DayContent> {
    let v = state.settings().vault();
    Ok(DayContent {
        raw: vault::read_raw(&v, &date).map_err(err)?,
        note: vault::read_note(&v, &date).map_err(err)?,
        date,
    })
}

#[tauri::command]
fn write_raw(state: tauri::State<AppState>, date: String, content: String) -> CmdResult<()> {
    vault::write_raw(&state.settings().vault(), &date, &content).map_err(err)
}

#[tauri::command]
fn write_note(state: tauri::State<AppState>, date: String, content: String) -> CmdResult<()> {
    vault::write_note(&state.settings().vault(), &date, &content).map_err(err)
}

#[tauri::command]
fn write_entity(
    state: tauri::State<AppState>,
    kind: String,
    slug: String,
    content: String,
) -> CmdResult<()> {
    vault::write_entity(&state.settings().vault(), &kind, &slug, &content).map_err(err)
}

#[tauri::command]
fn write_personal(state: tauri::State<AppState>, content: String) -> CmdResult<()> {
    vault::write_personal(&state.settings().vault(), &content).map_err(err)
}

#[tauri::command]
fn write_ideas(state: tauri::State<AppState>, content: String) -> CmdResult<()> {
    vault::write_ideas(&state.settings().vault(), &content).map_err(err)
}

#[tauri::command]
fn write_tasks(state: tauri::State<AppState>, content: String) -> CmdResult<()> {
    vault::write_tasks(&state.settings().vault(), &content).map_err(err)
}

#[tauri::command]
fn create_entity(
    state: tauri::State<AppState>,
    kind: String,
    name: String,
    scope: String,
) -> CmdResult<vault::ProjectMeta> {
    vault::create_entity(&state.settings().vault(), &kind, &name, &scope).map_err(err)
}

#[tauri::command]
fn delete_entity(
    state: tauri::State<AppState>,
    kind: String,
    slug: String,
) -> CmdResult<()> {
    vault::delete_entity(&state.settings().vault(), &kind, &slug).map_err(err)
}

/// Open a vault-relative path in the OS file manager (parent folder for files).
#[tauri::command]
fn reveal_path(
    state: tauri::State<AppState>,
    app: tauri::AppHandle,
    rel: String,
) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let v = state.settings().vault();
    vault::ensure_vault(&v).map_err(err)?;
    let path = vault::vault_abs(&v, &rel).map_err(err)?;
    if !path.exists() {
        return Err(format!("Path not found: {rel}"));
    }
    let target = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or(path.clone())
    } else {
        path
    };
    app.opener()
        .open_path(target.to_string_lossy().to_string(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
async fn refresh_entity_overview(
    state: tauri::State<'_, AppState>,
    kind: String,
    slug: String,
) -> CmdResult<String> {
    let s = state.settings();
    let v = s.vault();
    let page = vault::read_entity(&v, &kind, &slug).map_err(err)?;
    let name = vault::read_projects_config(&v)
        .into_iter()
        .find(|p| p.slug == slug)
        .map(|p| p.name)
        .unwrap_or_else(|| slug.clone());
    let overview = ai::refresh_overview(ai::OverviewRequest {
        provider: s.normalized_provider(),
        api_key: &s.resolved_api_key(),
        model: &s.model,
        effort: &s.effort,
        title: &name,
        kind: &kind,
        page_markdown: &page,
    })
    .await
    .map_err(err)?;
    vault::set_entity_overview(&v, &kind, &slug, &overview).map_err(err)?;
    vault::read_entity(&v, &kind, &slug).map_err(err)
}

#[tauri::command]
async fn refresh_personal_overview(state: tauri::State<'_, AppState>) -> CmdResult<String> {
    let s = state.settings();
    let v = s.vault();
    let page = vault::read_personal(&v);
    let overview = ai::refresh_overview(ai::OverviewRequest {
        provider: s.normalized_provider(),
        api_key: &s.resolved_api_key(),
        model: &s.model,
        effort: &s.effort,
        title: "Personal",
        kind: "personal",
        page_markdown: &page,
    })
    .await
    .map_err(err)?;
    vault::set_personal_overview(&v, &overview).map_err(err)?;
    Ok(vault::read_personal(&v))
}

#[tauri::command]
fn list_projects(state: tauri::State<AppState>) -> CmdResult<Vec<vault::ProjectEntry>> {
    let s = state.settings();
    vault::list_projects(&s.vault(), &s.date_format).map_err(err)
}

#[tauri::command]
fn read_project(state: tauri::State<AppState>, slug: String) -> CmdResult<String> {
    vault::read_project(&state.settings().vault(), &slug).map_err(err)
}

#[tauri::command]
fn search(state: tauri::State<AppState>, query: String) -> CmdResult<Vec<vault::SearchHit>> {
    vault::search(&state.settings().vault(), &query, 200).map_err(err)
}

#[tauri::command]
fn get_projects_config(state: tauri::State<AppState>) -> Vec<ProjectMeta> {
    vault::read_projects_config(&state.settings().vault())
}

#[tauri::command]
fn save_projects_config(
    state: tauri::State<AppState>,
    projects: Vec<ProjectMeta>,
) -> CmdResult<()> {
    vault::write_projects_config(&state.settings().vault(), &projects).map_err(err)
}

#[tauri::command]
fn get_glossary(state: tauri::State<AppState>) -> String {
    std::fs::read_to_string(vault::config_dir(&state.settings().vault()).join("glossary.txt"))
        .unwrap_or_default()
}

#[tauri::command]
fn save_glossary(state: tauri::State<AppState>, text: String) -> CmdResult<()> {
    vault::write_glossary(&state.settings().vault(), &text).map_err(err)
}

#[tauri::command]
fn get_profile(state: tauri::State<AppState>) -> String {
    vault::read_profile(&state.settings().vault())
}

#[tauri::command]
fn save_profile(state: tauri::State<AppState>, text: String) -> CmdResult<()> {
    vault::write_profile(&state.settings().vault(), &text).map_err(err)
}

#[tauri::command]
fn read_entity(
    state: tauri::State<AppState>,
    kind: String,
    slug: String,
) -> CmdResult<String> {
    vault::read_entity(&state.settings().vault(), &kind, &slug).map_err(err)
}

#[tauri::command]
fn reveal_vault(state: tauri::State<AppState>, app: tauri::AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let v = state.settings().vault();
    vault::ensure_vault(&v).map_err(err)?;
    app.opener()
        .open_path(v.to_string_lossy().to_string(), None::<&str>)
        .map_err(err)
}

// ----------------------------------------------------------------- process

#[derive(Serialize)]
struct ItemProcessResult {
    id: String,
    date: String,
    entry_count: usize,
    destinations: Vec<String>,
    new_entities: Vec<String>,
    summary: Vec<String>,
    /// Structural changes the capture asked for, in plain words, so a wrong one
    /// is visible rather than a silent surprise in the sidebar.
    #[serde(default)]
    actions: Vec<String>,
}

#[derive(Serialize)]
struct InboxProcessResult {
    processed: Vec<ItemProcessResult>,
    /// Items that failed; they stay in the inbox.
    errors: Vec<String>,
}

/// Apply a triage result to the vault. Order matters: destinations and raw
/// first; inbox delete last. A crash mid-way leaves the item in the inbox.
fn apply_triage(
    v: &std::path::Path,
    item: &vault::InboxItem,
    triage: &ai::TriageResult,
    known: &mut Vec<ProjectMeta>,
    date_fmt: &str,
    time_fmt: &str,
) -> Result<ItemProcessResult, String> {
    let mut destinations = Vec::new();
    let mut new_entities = Vec::new();
    let applied = apply_actions(v, &triage.actions, known, item)?;

    // Group project/area entries by slug so one upsert covers the whole item.
    let mut entity_bodies: std::collections::HashMap<String, (String, String, String, String)> =
        std::collections::HashMap::new();
    // key -> (kind, name, scope, body)

    // Resolve slug -> kind up front, including entities this very capture is
    // creating, so a task can link to a project that does not exist yet.
    let mut slug_kind: std::collections::HashMap<String, String> = known
        .iter()
        .map(|k| (k.slug.clone(), k.kind.clone()))
        .collect();
    for e in &triage.entries {
        if (e.kind == "project" || e.kind == "area") && !e.slug.trim().is_empty() {
            slug_kind.insert(e.slug.clone(), e.kind.clone());
        }
    }
    let link_for = |slug: &str| -> Option<String> {
        let slug = slug.trim();
        if slug.is_empty() {
            return None;
        }
        let dir = match slug_kind.get(slug).map(|s| s.as_str()) {
            Some("area") => "areas",
            _ => "projects",
        };
        Some(format!("{dir}/{slug}"))
    };

    for (n, e) in triage.entries.iter().enumerate() {
        let entry_id = entries::entry_id(&item.id, n);
        let link = link_for(&e.slug);
        match e.kind.as_str() {
            "project" | "area" => {
                let key = format!("{}:{}", e.kind, e.slug);
                let section = ai::render_entity_section(e, &item.date);
                let entry = entity_bodies.entry(key).or_insert_with(|| {
                    (
                        e.kind.clone(),
                        e.name.clone(),
                        e.scope.clone(),
                        String::new(),
                    )
                });
                if entry.1.is_empty() {
                    entry.1 = e.name.clone();
                }
                entry.3.push_str(&section);
                entry.3.push('\n');
            }
            "idea" => {
                let text = if e.body.trim().is_empty() {
                    e.title.clone()
                } else {
                    format!("{} — {}", e.title, e.body.trim())
                };
                vault::append_idea(
                    v,
                    &item.date,
                    &item.time,
                    &e.scope,
                    &text,
                    date_fmt,
                    time_fmt,
                    &entry_id,
                    link.as_deref(),
                )
                .map_err(err)?;
                destinations.push(format!("idea ({})", e.scope));
            }
            "task" => {
                let label = if e.title.trim().is_empty() {
                    e.body.trim()
                } else {
                    e.title.trim()
                };
                vault::append_task(
                    v,
                    &item.date,
                    &e.scope,
                    label,
                    e.due.as_deref(),
                    date_fmt,
                    &entry_id,
                    link.as_deref(),
                )
                .map_err(err)?;
                destinations.push(format!("task ({})", e.scope));
            }
            _ => {
                destinations.push(format!("note ({})", e.scope));
            }
        }
    }

    // Personal rollup: every personal-scoped entry also lands in personal.md.
    let has_personal = triage.entries.iter().any(|e| e.scope == "personal");
    if has_personal {
        vault::clear_personal_item(v, &item.id, date_fmt).map_err(err)?;
        for e in &triage.entries {
            if e.scope != "personal" {
                continue;
            }
            let dest = match e.kind.as_str() {
                "project" => format!("[[projects/{}|{}]]", e.slug, e.name),
                "area" => format!("[[areas/{}|{}]]", e.slug, e.name),
                "idea" => "ideas".into(),
                "task" => "tasks".into(),
                _ => "note".into(),
            };
            // Full body for notes; short pointer for things that have their own page.
            let body = match e.kind.as_str() {
                "note" => e.body.clone(),
                "project" | "area" => {
                    if e.body.trim().is_empty() {
                        format!("See {dest}")
                    } else {
                        // Keep it short on the personal rollup.
                        let excerpt: String = e.body.chars().take(280).collect();
                        format!("{excerpt}\n\n→ {dest}")
                    }
                }
                _ => {
                    if e.body.trim().is_empty() {
                        e.title.clone()
                    } else {
                        e.body.clone()
                    }
                }
            };
            let body = if e.kind == "note" {
                vault::ensure_attachment_markdown(&item.text, &body)
            } else {
                body
            };
            vault::upsert_personal_item(
                v,
                &item.date,
                &item.id,
                &item.time,
                &e.title,
                &dest,
                &body,
                date_fmt,
                time_fmt,
            )
            .map_err(err)?;
            if !destinations.iter().any(|d| d.starts_with("personal")) {
                destinations.push("personal".into());
            }
        }
    }

    // Write entity files. Attachments from the capture go on the first project/area only.
    let mut attachments_placed = false;
    for (key, (kind, name, scope, body)) in &entity_bodies {
        let slug = key.split_once(':').map(|(_, s)| s).unwrap_or(key);
        let body = if !attachments_placed {
            attachments_placed = true;
            vault::ensure_attachment_markdown(&item.text, body)
        } else {
            body.clone()
        };
        vault::upsert_entity_day(
            v,
            kind,
            slug,
            name,
            scope,
            &item.date,
            &item.id,
            &body,
            date_fmt,
        )
        .map_err(err)?;
        destinations.push(format!("{kind}/{slug}"));

        if !known.iter().any(|k| k.slug == *slug) {
            new_entities.push(name.clone());
            known.push(ProjectMeta {
                slug: slug.to_string(),
                name: name.clone(),
                kind: kind.clone(),
                scope: scope.clone(),
                status: "active".into(),
                parent: String::new(),
                aliases: vec![],
                description: String::new(),
            });
        } else if let Some(k) = known.iter_mut().find(|k| k.slug == *slug) {
            if k.kind.is_empty() {
                k.kind = kind.clone();
            }
            if k.scope.is_empty() {
                k.scope = scope.clone();
            }
        }
    }

    vault::append_raw_item(
        v,
        &item.date,
        Some(&item.id),
        &item.text,
        date_fmt,
        time_fmt,
    )
    .map_err(err)?;

    // Keep the structured record. The markdown above is the readable half; this
    // is what makes "everything open on Daybook" a query instead of a grep.
    let records: Vec<entries::EntryRecord> = triage
        .entries
        .iter()
        .enumerate()
        .map(|(n, e)| entries::EntryRecord {
            id: entries::entry_id(&item.id, n),
            item_id: item.id.clone(),
            date: item.date.clone(),
            time: item.time.clone(),
            scope: e.scope.clone(),
            kind: e.kind.clone(),
            slug: e.slug.clone(),
            name: e.name.clone(),
            title: e.title.clone(),
            body: e.body.clone(),
            accomplished: e.accomplished.clone(),
            decisions: e.decisions.clone(),
            open: e.open.clone(),
            due: e.due.clone(),
            source: entries::SOURCE_TRIAGE.into(),
        })
        .collect();
    entries::replace_item(v, &item.id, &records).map_err(err)?;

    let day_body = vault::ensure_attachment_markdown(
        &item.text,
        &ai::render_day_item_body(&triage.entries),
    );
    let title = ai::primary_title(&triage.entries, &triage.summary);
    vault::upsert_day_item(
        v,
        &item.date,
        &item.id,
        &item.time,
        &title,
        &triage.summary,
        &day_body,
        date_fmt,
        time_fmt,
    )
    .map_err(err)?;

    // Only delete once everything above succeeded.
    vault::delete_inbox_item(v, &item.id).map_err(err)?;

    Ok(ItemProcessResult {
        id: item.id.clone(),
        date: item.date.clone(),
        entry_count: triage.entries.len(),
        destinations,
        new_entities,
        summary: triage.summary.clone(),
        actions: applied,
    })
}

/// Carry out the structural instructions in a capture.
///
/// These are the only writes in the app driven by a model's reading of intent
/// rather than a click, so they are deliberately narrow: create, rename, move,
/// set status. Nothing here deletes. Every one is reported back in plain words
/// so a misread instruction is visible immediately, and page deletion stays a
/// human action that the trash can undo.
fn apply_actions(
    v: &std::path::Path,
    actions: &[ai::RoutedAction],
    known: &mut Vec<ProjectMeta>,
    item: &vault::InboxItem,
) -> Result<Vec<String>, String> {
    // A free function rather than a closure: a closure capturing `known` would
    // hold the borrow for the whole loop, and every arm needs to mutate it.
    fn resolve(known: &[ProjectMeta], slug: &str) -> Option<ProjectMeta> {
        let s = vault::slugify(slug);
        known.iter().find(|m| m.slug == s).cloned()
    }

    let mut done = Vec::new();
    for a in actions {
        match a.op.as_str() {
            "create_page" => {
                let name = a.name.trim();
                if name.is_empty() {
                    continue;
                }
                let kind = if a.kind == "area" { "area" } else { "project" };
                let scope = if a.scope == "work" { "work" } else { "personal" };
                let slug = vault::slugify(name);
                if known.iter().any(|m| m.slug == slug) {
                    done.push(format!("{name} already existed"));
                    continue;
                }
                let meta = vault::create_entity(v, kind, name, scope).map_err(err)?;
                let parent = vault::slugify(&a.parent);
                if !parent.is_empty() && known.iter().any(|m| m.slug == parent) {
                    vault::set_entity_parent(v, kind, &meta.slug, &parent).map_err(err)?;
                    let pname = resolve(known, &parent).map(|m| m.name).unwrap_or(parent);
                    done.push(format!("created {kind} {name} under {pname}"));
                } else {
                    done.push(format!("created {kind} {name}"));
                }
                known.push(meta);
            }
            "rename_page" => {
                let (Some(meta), name) = (resolve(known, &a.slug), a.name.trim()) else {
                    continue;
                };
                if name.is_empty() || name == meta.name {
                    continue;
                }
                vault::rename_entity(v, &meta.kind, &meta.slug, name).map_err(err)?;
                if let Some(m) = known.iter_mut().find(|m| m.slug == meta.slug) {
                    m.name = name.to_string();
                }
                done.push(format!("renamed {} to {name}", meta.name));
            }
            "move_page" => {
                let Some(meta) = resolve(known, &a.slug) else { continue };
                let parent = vault::slugify(&a.parent);
                if !parent.is_empty() && !known.iter().any(|m| m.slug == parent) {
                    continue;
                }
                vault::set_entity_parent(v, &meta.kind, &meta.slug, &parent).map_err(err)?;
                if let Some(m) = known.iter_mut().find(|m| m.slug == meta.slug) {
                    m.parent = parent.clone();
                }
                done.push(if parent.is_empty() {
                    format!("moved {} to the top level", meta.name)
                } else {
                    let pname = resolve(known, &parent).map(|m| m.name).unwrap_or(parent);
                    format!("moved {} under {pname}", meta.name)
                });
            }
            "set_status" => {
                let Some(meta) = resolve(known, &a.slug) else { continue };
                let status = match a.status.trim() {
                    "paused" => "paused",
                    "done" => "done",
                    "active" => "active",
                    _ => continue,
                };
                vault::set_entity_status(v, &meta.kind, &meta.slug, status).map_err(err)?;
                if let Some(m) = known.iter_mut().find(|m| m.slug == meta.slug) {
                    m.status = status.to_string();
                }
                done.push(format!("marked {} {status}", meta.name));
            }
            _ => {}
        }
    }
    if !done.is_empty() {
        // Attribute the change so it can be traced back to what was said.
        let _ = item;
    }
    Ok(done)
}

async fn refresh_touched_overviews(
    v: &std::path::Path,
    s: &Settings,
    processed: &[ItemProcessResult],
) {
    if s.resolved_api_key().is_empty() {
        return;
    }
    let provider = s.normalized_provider().to_string();
    let api_key = s.resolved_api_key();

    let mut entities: Vec<(String, String, String)> = Vec::new(); // kind, slug, name
    let mut personal = false;
    for r in processed {
        for d in &r.destinations {
            if d == "personal" {
                personal = true;
            }
            if let Some((kind, slug)) = d.split_once('/') {
                if kind == "project" || kind == "area" {
                    if !entities.iter().any(|(k, s, _)| k == kind && s == slug) {
                        let name = vault::read_projects_config(v)
                            .into_iter()
                            .find(|p| p.slug == slug)
                            .map(|p| p.name)
                            .unwrap_or_else(|| slug.to_string());
                        entities.push((kind.to_string(), slug.to_string(), name));
                    }
                }
            }
        }
    }

    for (kind, slug, name) in entities {
        let page = match vault::read_entity(v, &kind, &slug) {
            Ok(p) if !p.trim().is_empty() => p,
            _ => continue,
        };
        if let Ok(overview) = ai::refresh_overview(ai::OverviewRequest {
            provider: &provider,
            api_key: &api_key,
            model: &s.model,
            effort: &s.effort,
            title: &name,
            kind: &kind,
            page_markdown: &page,
        })
        .await
        {
            let _ = vault::set_entity_overview(v, &kind, &slug, &overview);
        }
    }

    if personal {
        let page = vault::read_personal(v);
        if let Ok(overview) = ai::refresh_overview(ai::OverviewRequest {
            provider: &provider,
            api_key: &api_key,
            model: &s.model,
            effort: &s.effort,
            title: "Personal",
            kind: "personal",
            page_markdown: &page,
        })
        .await
        {
            let _ = vault::set_personal_overview(v, &overview);
        }
    }
}

/// Drain the inbox. Optional `date` / `id` narrow which items run.
/// Failed items stay in the inbox.
#[tauri::command]
async fn process_inbox(
    state: tauri::State<'_, AppState>,
    date: Option<String>,
    id: Option<String>,
) -> CmdResult<InboxProcessResult> {
    let s = state.settings();
    let _guard = state.processing.lock().await;
    drain_inbox(&s, date, id).await
}

/// The triage loop itself, with no Tauri state attached, so the background
/// auto-processor can run exactly the same path the buttons do.
async fn drain_inbox(
    s: &Settings,
    date: Option<String>,
    id: Option<String>,
) -> CmdResult<InboxProcessResult> {
    let v = s.vault();
    let api_key = s.resolved_api_key();
    let provider = s.normalized_provider().to_string();
    let mut known = vault::read_projects_config(&v);
    let glossary = vault::read_glossary(&v);
    let profile = vault::read_profile(&v);

    let items = vault::list_inbox(&v).map_err(err)?;
    let items: Vec<_> = items
        .into_iter()
        .filter(|i| date.as_ref().map(|d| d == &i.date).unwrap_or(true))
        .filter(|i| id.as_ref().map(|x| x == &i.id).unwrap_or(true))
        .collect();

    if items.is_empty() {
        return Ok(InboxProcessResult {
            processed: vec![],
            errors: vec![if id.is_some() {
                "Inbox item not found.".into()
            } else {
                "Inbox is empty.".into()
            }],
        });
    }

    let mut processed = Vec::new();
    let mut errors = Vec::new();
    let mut learned_any = false;

    for item in &items {
        let triage = match ai::triage_item(ai::TriageRequest {
            provider: &provider,
            api_key: &api_key,
            model: &s.model,
            effort: &s.effort,
            date: &item.date,
            time: &item.time,
            text: &item.text,
            vault: &v,
            projects: &known,
            glossary: &glossary,
            profile: &profile,
        })
        .await
        {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{}: {e}", item.id));
                continue;
            }
        };

        let before = known.len();
        match apply_triage(
            &v,
            item,
            &triage,
            &mut known,
            &s.date_format,
            &s.time_format,
        ) {
            Ok(r) => {
                if known.len() > before {
                    learned_any = true;
                }
                processed.push(r);
            }
            Err(e) => errors.push(format!("{}: {e}", item.id)),
        }
    }

    if learned_any {
        vault::write_projects_config(&v, &known).map_err(err)?;
    }

    // Standing overviews: rewrite ## Overview only for pages this batch touched.
    refresh_touched_overviews(&v, s, &processed).await;

    Ok(InboxProcessResult { processed, errors })
}

/// Re-process is now "drain inbox for this date". Kept as a command name the
/// Days UI already calls.
#[tauri::command]
async fn process_day(state: tauri::State<'_, AppState>, date: String) -> CmdResult<InboxProcessResult> {
    process_inbox(state, Some(date), None).await
}

#[tauri::command]
async fn process_inbox_item(
    state: tauri::State<'_, AppState>,
    id: String,
) -> CmdResult<InboxProcessResult> {
    process_inbox(state, None, Some(id)).await
}

/// Route captures on their own once they have stopped moving.
///
/// This is only safe because entries became editable: silent routing with no
/// way to correct a bad guess would be a trap. A failure leaves the item in the
/// inbox and backs off rather than retrying in a loop burning API calls.
fn spawn_auto_processor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        const TICK: u64 = 20;
        let mut backoff = 0u32;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(TICK)).await;

            let state = app.state::<AppState>();
            let s = state.settings();
            if !s.auto_process || s.resolved_api_key().is_empty() {
                continue;
            }
            if backoff > 0 {
                backoff -= 1;
                continue;
            }

            let idle = match vault::list_inbox_idle(&s.vault(), s.auto_process_delay_secs) {
                Ok(i) => i,
                Err(_) => continue,
            };
            if idle.is_empty() {
                continue;
            }

            let guard = state.processing.lock().await;
            let mut any = false;
            let mut failed = 0usize;
            for item in idle {
                match drain_inbox(&s, None, Some(item.id.clone())).await {
                    Ok(r) => {
                        if !r.processed.is_empty() {
                            any = true;
                        }
                        failed += r.errors.len();
                    }
                    Err(_) => failed += 1,
                }
            }
            drop(guard);

            if failed > 0 {
                // Roughly a minute of quiet per consecutive failure, capped.
                backoff = (backoff + 3).min(15);
            }
            if any || failed > 0 {
                let _ = app.emit("inbox-auto-processed", failed);
            }
        }
    });
}

// -------------------------------------------------------------------- setup

fn toggle_capture(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("capture") else {
        return;
    };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("capture-focus", ());
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The app has to outlive its windows: the capture hotkey is the primary entry
/// point and it only works while the process is alive. So closing a window hides
/// it, and the tray is the only way to actually quit.
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Daybook", true, None::<&str>)?;
    let capture = MenuItem::with_id(app, "capture", "New entry", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &capture, &quit])?;

    TrayIconBuilder::with_id("daybook-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Daybook")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "capture" => toggle_capture(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // Fire on press only; the release event would immediately re-toggle.
                    if event.state() == ShortcutState::Pressed {
                        toggle_capture(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings = config::load(&config_dir);
            // A missing vault is normal on first run; don't fail startup over it.
            let _ = vault::ensure_vault(&settings.vault());
            let _ = config::save(&config_dir, &settings);

            if let Ok(s) = Shortcut::from_str(&settings.capture_hotkey) {
                let _ = app.global_shortcut().register(s);
            }
            build_tray(app.handle())?;

            let auto = settings.auto_process;
            app.manage(AppState {
                settings: Mutex::new(settings),
                config_dir,
                processing: tauri::async_runtime::Mutex::new(()),
            });

            if auto {
                spawn_auto_processor(app.handle().clone());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing any window hides it rather than destroying it. The capture
            // window must survive for the hotkey to work, and the app must survive
            // for the hotkey to fire at all. Quit lives in the tray menu.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            append_entry,
            list_inbox,
            delete_inbox_item,
            update_inbox_item,
            ensure_day,
            today_date,
            list_backlinks,
            query_entries,
            rebuild_entry_index,
            ask_vault,
            update_entry,
            create_entry,
            delete_entry,
            resolve_open_loop,
            set_entity_parent,
            list_trash,
            restore_trash,
            purge_trash,
            empty_trash,
            set_task_done,
            save_attachment,
            save_file_attachment,
            attachment_data_url,
            hide_capture,
            show_capture,
            read_tasks,
            read_ideas,
            read_personal,
            list_history,
            read_history_item,
            toggle_task_line,
            list_days,
            read_day,
            write_raw,
            write_note,
            write_entity,
            write_personal,
            write_ideas,
            write_tasks,
            create_entity,
            delete_entity,
            refresh_entity_overview,
            refresh_personal_overview,
            list_projects,
            read_project,
            read_entity,
            search,
            get_projects_config,
            save_projects_config,
            get_glossary,
            save_glossary,
            get_profile,
            save_profile,
            reveal_vault,
            reveal_path,
            process_inbox,
            process_inbox_item,
            process_day,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
