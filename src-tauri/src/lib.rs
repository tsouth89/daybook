mod ai;
mod config;
mod vault;

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
    vault::delete_inbox_item(&state.settings().vault(), &id).map_err(err)
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
fn list_projects(state: tauri::State<AppState>) -> CmdResult<Vec<vault::ProjectEntry>> {
    vault::list_projects(&state.settings().vault()).map_err(err)
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
) -> Result<ItemProcessResult, String> {
    let mut destinations = Vec::new();
    let mut new_entities = Vec::new();

    // Group project/area entries by slug so one upsert covers the whole item.
    let mut entity_bodies: std::collections::HashMap<String, (String, String, String, String)> =
        std::collections::HashMap::new();
    // key -> (kind, name, scope, body)

    for e in &triage.entries {
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
                vault::append_idea(v, &item.date, &item.time, &e.scope, &text).map_err(err)?;
                destinations.push(format!("idea ({})", e.scope));
            }
            "task" => {
                let label = if e.title.trim().is_empty() {
                    e.body.trim()
                } else {
                    e.title.trim()
                };
                vault::append_task(v, &item.date, &e.scope, label, e.due.as_deref())
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
        vault::clear_personal_item(v, &item.id).map_err(err)?;
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
        vault::upsert_entity_day(v, kind, slug, name, scope, &item.date, &item.id, &body)
            .map_err(err)?;
        destinations.push(format!("{kind}/{slug}"));

        if !known.iter().any(|k| k.slug == *slug) {
            new_entities.push(name.clone());
            known.push(ProjectMeta {
                slug: slug.to_string(),
                name: name.clone(),
                kind: kind.clone(),
                scope: scope.clone(),
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

    vault::append_raw_item(v, &item.date, Some(&item.id), &item.text).map_err(err)?;

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
    })
}

/// Drain the inbox. If `date` is set, only process items from that day.
/// Failed items stay in the inbox.
#[tauri::command]
async fn process_inbox(
    state: tauri::State<'_, AppState>,
    date: Option<String>,
) -> CmdResult<InboxProcessResult> {
    let s = state.settings();
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
        .collect();

    if items.is_empty() {
        return Ok(InboxProcessResult {
            processed: vec![],
            errors: vec!["Inbox is empty.".into()],
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
        match apply_triage(&v, item, &triage, &mut known) {
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

    Ok(InboxProcessResult { processed, errors })
}

/// Re-process is now "drain inbox for this date". Kept as a command name the
/// Days UI already calls.
#[tauri::command]
async fn process_day(state: tauri::State<'_, AppState>, date: String) -> CmdResult<InboxProcessResult> {
    process_inbox(state, Some(date)).await
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

            app.manage(AppState {
                settings: Mutex::new(settings),
                config_dir,
            });
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
            save_attachment,
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
            process_inbox,
            process_day,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
