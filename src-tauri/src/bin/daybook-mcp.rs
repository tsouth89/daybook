//! MCP server for Daybook.
//!
//! A separate binary rather than something hosted inside the app, because the
//! vault is already the API: plain Markdown plus a regenerable index. That
//! means an assistant can work whether or not Daybook is open, and there is no
//! second source of truth to keep in step.
//!
//! Transport is newline-delimited JSON-RPC on stdio, which is what MCP clients
//! launch. The protocol surface needed here is small and stable — initialize,
//! tools/list, tools/call — so it is implemented directly rather than through
//! an SDK whose churn would outweigh the ~150 lines it saves.
//!
//! Writes go through the same functions the app uses, so trash, task-line
//! rewriting, and index invariants all hold. Nothing here deletes a page.

use daybook_lib::{config, entries, vault};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req): Result<Value, _> = serde_json::from_str(line) else {
            continue;
        };

        // Notifications carry no id and must not be answered.
        let Some(id) = req.get("id").cloned() else {
            continue;
        };
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));

        let response = match dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(message) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32000, "message": message }
            }),
        };

        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }
}

fn dispatch(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "daybook", "version": env!("CARGO_PKG_VERSION") }
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or("tools/call needs a name")?;
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call(name, &args) {
                // A tool failing is a result the model should see and react to,
                // not a transport error that aborts the exchange.
                Ok(text) => Ok(json!({ "content": [{ "type": "text", "text": text }] })),
                Err(e) => Ok(json!({
                    "content": [{ "type": "text", "text": e }],
                    "isError": true
                })),
            }
        }
        other => Err(format!("unknown method: {other}")),
    }
}

fn vault_path() -> Result<std::path::PathBuf, String> {
    // DAYBOOK_VAULT wins, so a second vault can be served without touching the
    // app's settings — and so this is testable without writing into a real one.
    if let Some(p) = std::env::var_os("DAYBOOK_VAULT") {
        let v = std::path::PathBuf::from(p);
        vault::ensure_vault(&v).map_err(|e| e.to_string())?;
        return Ok(v);
    }
    // Otherwise the same settings file the app writes, so both see one vault.
    let dir = dirs_config().ok_or("could not find the Daybook config directory")?;
    let settings = config::load(&dir);
    let v = settings.vault();
    if !v.exists() {
        return Err(format!("vault not found at {}", v.display()));
    }
    Ok(v)
}

fn dirs_config() -> Option<std::path::PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .map(|h| h.join(".config"))
            })
    };
    base.map(|b| b.join("com.tsouth.daybook"))
}

fn s(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn tools() -> Vec<Value> {
    let str_prop = |desc: &str| json!({ "type": "string", "description": desc });
    vec![
        json!({
            "name": "daybook_capture",
            "description": "Drop text into the Daybook inbox exactly as written. Daybook's own \
                triage splits and files it. This is the right tool for anything the user says \
                to record, log, or note down.",
            "inputSchema": {
                "type": "object",
                "required": ["text"],
                "properties": { "text": str_prop("What to record, in the user's own words.") }
            }
        }),
        json!({
            "name": "daybook_search",
            "description": "Search routed entries by text and properties. Use this before \
                answering anything about what the user has previously worked on or decided.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": str_prop("Words to match."),
                    "project": str_prop("Project or area slug."),
                    "kind": str_prop("project | area | task | idea | note"),
                    "scope": str_prop("personal | work"),
                    "open_only": { "type": "boolean", "description": "Only unresolved loops." },
                    "undone_only": { "type": "boolean", "description": "Only unfinished tasks." }
                }
            }
        }),
        json!({
            "name": "daybook_list_projects",
            "description": "List every project and area, with status, parent, and last activity.",
            "inputSchema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "daybook_read_page",
            "description": "Read a project or area page as Markdown.",
            "inputSchema": {
                "type": "object",
                "required": ["slug"],
                "properties": {
                    "slug": str_prop("Project or area slug."),
                    "kind": str_prop("project or area; defaults to project.")
                }
            }
        }),
        json!({
            "name": "daybook_add_task",
            "description": "Add a task, optionally belonging to a project and with a due date.",
            "inputSchema": {
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": str_prop("Short task label."),
                    "project": str_prop("Owning project or area slug."),
                    "due": str_prop("YYYY-MM-DD."),
                    "scope": str_prop("personal | work")
                }
            }
        }),
        json!({
            "name": "daybook_complete_task",
            "description": "Tick a task off by its entry id, as returned by daybook_search.",
            "inputSchema": {
                "type": "object",
                "required": ["entry_id"],
                "properties": {
                    "entry_id": str_prop("Entry id."),
                    "done": { "type": "boolean", "description": "Defaults to true." }
                }
            }
        }),
        json!({
            "name": "daybook_create_page",
            "description": "Create a project or area page, optionally nested under another.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": str_prop("Display name."),
                    "kind": str_prop("project or area; defaults to project."),
                    "scope": str_prop("personal | work"),
                    "parent": str_prop("Slug of the page this nests under.")
                }
            }
        }),
        json!({
            "name": "daybook_append_to_page",
            "description": "Append dated notes to a project or area page under today's heading.",
            "inputSchema": {
                "type": "object",
                "required": ["slug", "text"],
                "properties": {
                    "slug": str_prop("Project or area slug."),
                    "text": str_prop("Markdown to append."),
                    "kind": str_prop("project or area; defaults to project.")
                }
            }
        }),
    ]
}

fn call(name: &str, args: &Value) -> Result<String, String> {
    let v = vault_path()?;
    let settings = dirs_config()
        .map(|d| config::load(&d))
        .unwrap_or_default();
    let date_fmt = settings.date_format.clone();

    match name {
        "daybook_capture" => {
            let text = s(args, "text");
            if text.trim().is_empty() {
                return Err("Nothing to capture.".into());
            }
            let id = vault::write_inbox_item(&v, &text).map_err(|e| e.to_string())?;
            Ok(format!(
                "Captured as {id}. Daybook will file it automatically; it is in the inbox until then."
            ))
        }

        "daybook_search" => {
            let q = entries::EntryQuery {
                text: Some(s(args, "text")).filter(|t| !t.is_empty()),
                slug: Some(s(args, "project")).filter(|t| !t.is_empty()),
                kind: Some(s(args, "kind")).filter(|t| !t.is_empty()),
                scope: Some(s(args, "scope")).filter(|t| !t.is_empty()),
                open_only: args.get("open_only").and_then(|b| b.as_bool()).unwrap_or(false),
                undone_only: args
                    .get("undone_only")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
                limit: Some(40),
                ..Default::default()
            };
            let found = entries::query(&v, &q);
            if found.is_empty() {
                return Ok("No matching entries.".into());
            }
            let mut out = String::new();
            for e in &found {
                let r = &e.record;
                out.push_str(&format!("[{}] {} · {}", r.date, r.kind, r.title));
                if !r.slug.is_empty() {
                    let owner = if r.name.trim().is_empty() { &r.slug } else { &r.name };
                    out.push_str(&format!(" · {owner}"));
                }
                if r.kind == "task" {
                    out.push_str(if e.done { " · done" } else { " · open" });
                }
                out.push_str(&format!(" · id={}\n", r.id));
                if !r.open.is_empty() {
                    out.push_str(&format!("    open: {}\n", r.open.join("; ")));
                }
                if !r.decisions.is_empty() {
                    out.push_str(&format!("    decided: {}\n", r.decisions.join("; ")));
                }
            }
            Ok(out)
        }

        "daybook_list_projects" => {
            let projects = vault::list_projects(&v, &date_fmt).map_err(|e| e.to_string())?;
            if projects.is_empty() {
                return Ok("No projects yet.".into());
            }
            let mut out = String::new();
            for p in projects {
                out.push_str(&format!("{} ({}) · {}", p.name, p.slug, p.kind));
                if !p.parent.is_empty() {
                    out.push_str(&format!(" · under {}", p.parent));
                }
                out.push_str(&format!(" · {} · {}\n", p.status, p.scope));
            }
            Ok(out)
        }

        "daybook_read_page" => {
            let kind = s(args, "kind");
            let kind = if kind == "area" { "area" } else { "project" };
            let text = vault::read_entity(&v, kind, &s(args, "slug")).map_err(|e| e.to_string())?;
            if text.trim().is_empty() {
                Err("That page is empty or does not exist.".into())
            } else {
                Ok(text)
            }
        }

        "daybook_add_task" => {
            let title = s(args, "title");
            if title.trim().is_empty() {
                return Err("A task needs a title.".into());
            }
            let scope = s(args, "scope");
            let record = entries::EntryRecord {
                id: String::new(),
                item_id: String::new(),
                date: vault::today(),
                time: String::new(),
                scope: if scope == "work" { "work".into() } else { "personal".into() },
                kind: "task".into(),
                slug: s(args, "project"),
                name: String::new(),
                title: title.clone(),
                body: String::new(),
                accomplished: vec![],
                decisions: vec![],
                open: vec![],
                due: Some(s(args, "due")).filter(|d| !d.is_empty()),
                source: entries::SOURCE_TRIAGE.into(),
            };
            let made = entries::create_entry(&v, record, &date_fmt).map_err(|e| e.to_string())?;
            Ok(format!("Added task \"{}\" (id={}).", made.title, made.id))
        }

        "daybook_complete_task" => {
            let done = args.get("done").and_then(|b| b.as_bool()).unwrap_or(true);
            entries::set_task_done(&v, &s(args, "entry_id"), done).map_err(|e| e.to_string())?;
            Ok(if done { "Marked done.".into() } else { "Reopened.".to_string() })
        }

        "daybook_create_page" => {
            let name = s(args, "name");
            if name.trim().is_empty() {
                return Err("A page needs a name.".into());
            }
            let kind = s(args, "kind");
            let kind = if kind == "area" { "area" } else { "project" };
            let scope = s(args, "scope");
            let scope = if scope == "work" { "work" } else { "personal" };
            let meta = vault::create_entity(&v, kind, &name, scope).map_err(|e| e.to_string())?;
            let parent = s(args, "parent");
            if !parent.is_empty() {
                vault::set_entity_parent(&v, kind, &meta.slug, &parent)
                    .map_err(|e| e.to_string())?;
                return Ok(format!("Created {kind} \"{name}\" ({}) under {parent}.", meta.slug));
            }
            Ok(format!("Created {kind} \"{name}\" ({}).", meta.slug))
        }

        "daybook_append_to_page" => {
            let text = s(args, "text");
            if text.trim().is_empty() {
                return Err("Nothing to append.".into());
            }
            let kind = s(args, "kind");
            let kind = if kind == "area" { "area" } else { "project" };
            let slug = s(args, "slug");
            let projects = vault::read_projects_config(&v);
            let meta = projects.iter().find(|m| m.slug == slug);
            let name = meta.map(|m| m.name.clone()).unwrap_or_else(|| slug.clone());
            let scope = meta.map(|m| m.scope.clone()).unwrap_or_else(|| "personal".into());
            let date = vault::today();
            // A distinct item id keeps this splice separate from triage's, so
            // appending here never overwrites a section the app wrote.
            let item_id = format!("mcp-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
            vault::upsert_entity_day(
                &v, kind, &slug, &name, &scope, &date, &item_id, &text, &date_fmt,
            )
            .map_err(|e| e.to_string())?;
            Ok(format!("Appended to {name} under {date}."))
        }

        other => Err(format!("unknown tool: {other}")),
    }
}
