use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::vault::ProjectMeta;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";

// ---------------------------------------------------------------------------
// Triage: one inbox dump → discrete entries with scope × destination.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutedEntry {
    /// "personal" or "work"
    pub scope: String,
    /// "project" | "area" | "idea" | "task" | "note"
    pub kind: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_new: bool,
    pub title: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriageResult {
    pub summary: Vec<String>,
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

fn json_example() -> &'static str {
    r#"{
  "summary": ["Shipped inbox routing for Daybook", "Book dentist next week"],
  "entries": [
    {
      "scope": "work",
      "kind": "project",
      "slug": "daybook",
      "name": "Daybook",
      "is_new": false,
      "title": "Inbox routing",
      "body": "Got the inbox layer wiring so captures split into discrete entries.",
      "accomplished": ["Wired inbox triage"],
      "decisions": [],
      "open": [],
      "due": null
    },
    {
      "scope": "personal",
      "kind": "task",
      "slug": "",
      "name": "",
      "is_new": false,
      "title": "Book dentist",
      "body": "Need to schedule a cleaning next week.",
      "accomplished": [],
      "decisions": [],
      "open": [],
      "due": null
    }
  ]
}"#
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
         rather than transcribe.\n\n\
         Respond with a single JSON object matching this schema example:\n",
    );
    s.push_str(json_example());
    s.push_str("\n\n");

    let profile = profile.trim();
    if !profile.is_empty()
        && !profile.lines().all(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('#') || t == "-"
        })
    {
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
        "Triage this capture from {date} at {time}. Split it into discrete entries and route each \
         one. Return JSON only.\n\n<raw>\n{}\n</raw>",
        text.trim()
    )
}

pub struct TriageRequest<'a> {
    pub provider: &'a str,
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
        let hint = match req.provider {
            "deepseek" => "DeepSeek API key (or DEEPSEEK_API_KEY)",
            "anthropic" => "Anthropic API key (or ANTHROPIC_API_KEY)",
            _ => "OpenAI API key (or OPENAI_API_KEY)",
        };
        bail!("No {hint} set. Add one in Settings.");
    }
    if req.text.trim().is_empty() {
        bail!("Nothing to triage: capture is empty.");
    }

    let json_text = match req.provider {
        "deepseek" => call_openai_compatible(&req, DEEPSEEK_URL, false).await?,
        "anthropic" => call_anthropic(&req).await?,
        _ => call_openai_compatible(&req, OPENAI_URL, true).await?,
    };

    parse_triage_json(&json_text)
}

fn normalize_result(mut result: TriageResult) -> TriageResult {
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
    result
}

fn parse_triage_json(json_text: &str) -> Result<TriageResult> {
    let trimmed = json_text.trim();
    // Models occasionally wrap JSON in fences despite instructions.
    let trimmed = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s).trim())
        .unwrap_or(trimmed);

    let result: TriageResult = serde_json::from_str(trimmed)
        .map_err(|e| anyhow!("Could not parse the model's JSON: {e}\n---\n{trimmed}"))?;
    Ok(normalize_result(result))
}

async fn call_anthropic(req: &TriageRequest<'_>) -> Result<String> {
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
        .post(ANTHROPIC_URL)
        .header("x-api-key", req.api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        bail!(
            "Anthropic API error {}: {}",
            status.as_u16(),
            extract_error_message(&text)
        );
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

    v["content"]
        .as_array()
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
        })
        .map(str::to_string)
        .ok_or_else(|| anyhow!("No text block in the Anthropic response"))
}

/// OpenAI Chat Completions (also DeepSeek's OpenAI-compatible endpoint).
/// `strict_schema`: OpenAI supports json_schema; DeepSeek only supports json_object.
async fn call_openai_compatible(
    req: &TriageRequest<'_>,
    url: &str,
    strict_schema: bool,
) -> Result<String> {
    let system = system_prompt(req.projects, req.glossary, req.profile);
    let user = user_prompt(req.date, req.time, req.text);

    let response_format = if strict_schema {
        json!({
            "type": "json_schema",
            "json_schema": {
                "name": "daybook_triage",
                "strict": true,
                "schema": triage_schema()
            }
        })
    } else {
        json!({ "type": "json_object" })
    };

    let mut body = json!({
        "model": req.model,
        "max_tokens": 8000,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ],
        "response_format": response_format
    });

    if !strict_schema {
        // DeepSeek: thinking is on by default and costs output tokens. Triage
        // does not need it — keep Flash cheap and fast.
        body["thinking"] = json!({ "type": "disabled" });
    } else {
        // GPT-5.6 family accepts reasoning effort on chat completions.
        let effort = match req.effort {
            "low" | "minimal" | "none" => "low",
            "high" | "xhigh" | "max" => "high",
            _ => "medium",
        };
        body["reasoning_effort"] = json!(effort);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let resp = client
        .post(url)
        .header("authorization", format!("Bearer {}", req.api_key))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        let label = if strict_schema { "OpenAI" } else { "DeepSeek" };
        bail!(
            "{label} API error {}: {}",
            status.as_u16(),
            extract_error_message(&text)
        );
    }

    let v: serde_json::Value = serde_json::from_str(&text)?;
    let finish = v["choices"][0]["finish_reason"].as_str().unwrap_or("");
    if finish == "length" {
        bail!("Response hit the token limit. Try splitting this capture into smaller dumps.");
    }

    v["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("No content in the chat completion response"))
}

fn extract_error_message(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| {
            v["error"]["message"]
                .as_str()
                .map(str::to_string)
                .or_else(|| v["message"].as_str().map(str::to_string))
        })
        .unwrap_or_else(|| text.chars().take(500).collect())
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
