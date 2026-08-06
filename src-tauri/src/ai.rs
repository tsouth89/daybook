use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::vault::ProjectMeta;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

// ---------------------------------------------------------------------------
// Triage: one inbox dump → discrete entries with scope × destination.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutedEntry {
    /// "personal" or "work"
    pub scope: String,
    /// "project" | "area" | "idea" | "task" | "note"
    /// note = day-only; no dedicated destination file.
    pub kind: String,
    /// Required for project/area. kebab-case.
    #[serde(default)]
    pub slug: String,
    /// Display name for project/area.
    #[serde(default)]
    pub name: String,
    /// True when this project/area is not in the known list.
    #[serde(default)]
    pub is_new: bool,
    /// Short title for this discrete entry.
    pub title: String,
    /// Cleaned body for this entry, in the author's voice.
    pub body: String,
    #[serde(default)]
    pub accomplished: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub open: Vec<String>,
    /// Optional due date YYYY-MM-DD for tasks/appointments.
    #[serde(default)]
    pub due: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriageResult {
    /// One-to-three glance bullets covering this dump.
    pub summary: Vec<String>,
    /// Discrete things found in the dump. Empty only if the dump is empty.
    pub entries: Vec<RoutedEntry>,
}

fn triage_schema() -> serde_json::Value {
    let str_array = json!({ "type": "array", "items": { "type": "string" } });
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "entries"],
        "properties": {
            "summary": str_array,
            "entries": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "scope", "kind", "slug", "name", "is_new",
                        "title", "body", "accomplished", "decisions", "open", "due"
                    ],
                    "properties": {
                        "scope": { "type": "string" },
                        "kind": { "type": "string" },
                        "slug": { "type": "string" },
                        "name": { "type": "string" },
                        "is_new": { "type": "boolean" },
                        "title": { "type": "string" },
                        "body": { "type": "string" },
                        "accomplished": str_array,
                        "decisions": str_array,
                        "open": str_array,
                        "due": { "type": ["string", "null"] }
                    }
                }
            }
        }
    })
}

fn system_prompt(projects: &[ProjectMeta], glossary: &[String], profile: &str) -> String {
    let mut s = String::new();
    s.push_str(
        "You triage one capture from a personal daybook. The input is raw dictation or pasted \
         text: run-on sentences, false starts, no punctuation, transcription errors, and often \
         several unrelated things in one dump.\n\n\
         Your job is to split the dump into discrete entries and route each one. Three rules:\n\n\
         1. Never invent. Ambiguity stays ambiguous. Empty arrays are correct. A thin dump stays thin.\n\
         2. Preserve their voice in each entry's `body`. Fix transcription, add punctuation and \
         paragraph breaks, drop filler and false starts. Do not upgrade vocabulary or make it sound \
         professional.\n\
         3. Split aggressively. One dump that mentions a bug fix, a dentist appointment, and a \
         side idea becomes three entries. Do not stuff unrelated things into one entry.\n\n\
         Scope and destination are independent axes:\n\
         - `scope` is `personal` or `work`.\n\
         - `kind` is where it files:\n\
           - `project` — something with an end state (\"ship the BMX site\").\n\
           - `area` — ongoing responsibility with no end state (health, finances, the house).\n\
           - `idea` — maybe-someday, not actionable yet.\n\
           - `task` — a discrete open action or appointment.\n\
           - `note` — worth keeping on the day, nowhere else (a mood, a random observation).\n\n\
         Match projects/areas against the known list below, including aliases. Speech is loose. \
         If work clearly belongs to something not on the list, create it with a kebab-case slug \
         and set is_new to true. Prefer `area` over inventing a fake project for ongoing life domains.\n\n\
         For project/area entries: fill `accomplished` / `decisions` / `open` when present. \
         Decisions should keep the reason if one was given. For tasks, set `due` to YYYY-MM-DD \
         when a date is clear; otherwise null. `title` is a short label (a few words).\n\n\
         `summary` is 1–3 plain glance bullets for the whole dump — the only place you write \
         rather than transcribe.\n\n",
    );

    let profile = profile.trim();
    if !profile.is_empty() && !profile.lines().all(|l| {
        let t = l.trim();
        t.is_empty() || t.starts_with('#') || t == "-"
    }) {
        s.push_str("Author profile (durable context — do not echo it back):\n");
        s.push_str(profile);
        s.push_str("\n\n");
    }

    if projects.is_empty() {
        s.push_str(
            "Known projects/areas: none configured yet. Infer them from the dump and set is_new.\n\n",
        );
    } else {
        s.push_str("Known projects/areas:\n");
        for p in projects {
            s.push_str(&format!(
                "- {} (slug: {}, kind: {}, scope: {})",
                p.name, p.slug, p.kind, p.scope
            ));
            if !p.aliases.is_empty() {
                s.push_str(&format!(" [also called: {}]", p.aliases.join(", ")));
            }
            if !p.description.is_empty() {
                s.push_str(&format!(" — {}", p.description));
            }
            s.push('\n');
        }
        s.push('\n');
    }

    if !glossary.is_empty() {
        s.push_str(
            "Glossary. Dictation mangles these constantly. Phonetically close phrases are almost \
             certainly these terms — correct them:\n",
        );
        for g in glossary {
            s.push_str(&format!("- {g}\n"));
        }
        s.push('\n');
    }

    s
}

fn user_prompt(date: &str, time: &str, text: &str) -> String {
    format!(
        "Triage this capture from {date} at {time}. Split it into discrete entries and route each one.\n\n\
         <raw>\n{}\n</raw>",
        text.trim()
    )
}

pub struct TriageRequest<'a> {
    pub api_key: &'a str,
    pub model: &'a str,
    pub effort: &'a str,
    pub date: &'a str,
    pub time: &'a str,
    pub text: &'a str,
    pub projects: &'a [ProjectMeta],
    pub glossary: &'a [String],
    pub profile: &'a str,
}

pub async fn triage_item(req: TriageRequest<'_>) -> Result<TriageResult> {
    if req.api_key.is_empty() {
        bail!("No Anthropic API key set. Add one in Settings (or set ANTHROPIC_API_KEY).");
    }
    if req.text.trim().is_empty() {
        bail!("Nothing to triage: capture is empty.");
    }

    let body = json!({
        "model": req.model,
        "max_tokens": 8000,
        "system": system_prompt(req.projects, req.glossary, req.profile),
        "output_config": {
            "effort": req.effort,
            "format": { "type": "json_schema", "schema": triage_schema() }
        },
        "messages": [
            { "role": "user", "content": user_prompt(req.date, req.time, req.text) }
        ]
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let resp = client
        .post(API_URL)
        .header("x-api-key", req.api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        let msg = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
            .unwrap_or_else(|| text.chars().take(500).collect());
        bail!("Anthropic API error {}: {}", status.as_u16(), msg);
    }

    let v: serde_json::Value = serde_json::from_str(&text)?;
    if v["stop_reason"] == "refusal" {
        let category = v["stop_details"]["category"]
            .as_str()
            .unwrap_or("unspecified");
        bail!("The model declined to process this entry (category: {category}).");
    }
    if v["stop_reason"] == "max_tokens" {
        bail!("Response hit the token limit. Try splitting this capture into smaller dumps.");
    }

    let json_text = v["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .ok_or_else(|| anyhow!("No text block in the API response"))?;

    let mut result: TriageResult = serde_json::from_str(json_text)
        .map_err(|e| anyhow!("Could not parse the model's JSON: {e}"))?;

    // Normalize enums so routing doesn't choke on model drift.
    for e in &mut result.entries {
        e.scope = match e.scope.to_lowercase().as_str() {
            "personal" => "personal".into(),
            _ => "work".into(),
        };
        e.kind = match e.kind.to_lowercase().as_str() {
            "area" => "area".into(),
            "idea" => "idea".into(),
            "task" => "task".into(),
            "note" => "note".into(),
            _ => "project".into(),
        };
        if e.kind == "project" || e.kind == "area" {
            if e.slug.trim().is_empty() {
                e.slug = crate::vault::slugify(&e.name);
            } else {
                e.slug = crate::vault::slugify(&e.slug);
            }
            if e.name.trim().is_empty() {
                e.name = e.slug.clone();
            }
        }
        if let Some(due) = e.due.as_deref() {
            if chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d").is_err() {
                e.due = None;
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Rendering helpers used when writing destination files.
// ---------------------------------------------------------------------------

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|i| format!("- {}\n", i.trim()))
        .collect::<String>()
}

/// Block written under `## <date>` in a project/area file.
pub fn render_entity_section(e: &RoutedEntry, date: &str) -> String {
    let mut s = String::new();
    if !e.title.trim().is_empty() {
        s.push_str(&format!("**{}**\n\n", e.title.trim()));
    }
    if !e.accomplished.is_empty() {
        s.push_str("**Accomplished**\n\n");
        s.push_str(&bullets(&e.accomplished));
        s.push('\n');
    }
    if !e.decisions.is_empty() {
        s.push_str("**Decided**\n\n");
        s.push_str(&bullets(&e.decisions));
        s.push('\n');
    }
    if !e.open.is_empty() {
        s.push_str("**Open**\n\n");
        s.push_str(&bullets(&e.open));
        s.push('\n');
    }
    if !e.body.trim().is_empty() {
        s.push_str(e.body.trim());
        s.push_str("\n\n");
    }
    s.push_str(&format!("[[days/{date}]]\n"));
    s
}

/// Body for the day-note section covering one capture.
pub fn render_day_item_body(entries: &[RoutedEntry]) -> String {
    let mut s = String::new();
    for e in entries {
        let dest = match e.kind.as_str() {
            "project" => format!("[[projects/{}|{}]]", e.slug, e.name),
            "area" => format!("[[areas/{}|{}]]", e.slug, e.name),
            "idea" => "ideas".into(),
            "task" => "tasks".into(),
            _ => "note".into(),
        };
        s.push_str(&format!("### {} · {} · {}\n\n", e.title, e.scope, dest));
        if !e.body.trim().is_empty() {
            s.push_str(e.body.trim());
            s.push_str("\n\n");
        }
        if e.kind == "task" {
            if let Some(due) = &e.due {
                s.push_str(&format!("Due: {due}\n\n"));
            }
        }
    }
    s
}

pub fn primary_title(entries: &[RoutedEntry], summary: &[String]) -> String {
    if let Some(e) = entries.first() {
        if !e.title.trim().is_empty() {
            return e.title.trim().to_string();
        }
    }
    if let Some(b) = summary.first() {
        return b.chars().take(80).collect();
    }
    "Entry".into()
}
