use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App settings. Stored as JSON next to the app's config dir, NOT in the vault,
/// so the vault stays portable and contains no secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: String,
    /// Anthropic API key. Empty until the user sets it in Settings.
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    /// low | medium | high | xhigh | max
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default = "default_hotkey")]
    pub capture_hotkey: String,
    /// How many previous day-summaries to feed in as continuity context.
    #[serde(default = "default_context_days")]
    pub context_days: usize,
}

fn default_model() -> String {
    "claude-opus-5".into()
}
fn default_effort() -> String {
    // Summarising a day of dictation is not an intelligence-sensitive task.
    // medium keeps it fast and cheap; bump to high in Settings if output is thin.
    "medium".into()
}
fn default_hotkey() -> String {
    "CmdOrControl+Shift+Space".into()
}
fn default_context_days() -> usize {
    3
}

impl Default for Settings {
    fn default() -> Self {
        let vault = dirs_home()
            .map(|h| h.join("Journal"))
            .unwrap_or_else(|| PathBuf::from("Journal"));
        Settings {
            vault_path: vault.to_string_lossy().into_owned(),
            api_key: String::new(),
            model: default_model(),
            effort: default_effort(),
            capture_hotkey: default_hotkey(),
            context_days: default_context_days(),
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

impl Settings {
    pub fn vault(&self) -> PathBuf {
        PathBuf::from(&self.vault_path)
    }

    /// Falls back to the environment so you can run without storing a key on disk.
    pub fn resolved_api_key(&self) -> String {
        if !self.api_key.trim().is_empty() {
            return self.api_key.trim().to_string();
        }
        std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
    }
}

pub fn settings_path(config_dir: &PathBuf) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load(config_dir: &PathBuf) -> Settings {
    let path = settings_path(config_dir);
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default()
}

pub fn save(config_dir: &PathBuf, settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(config_dir).context("creating config dir")?;
    let path = settings_path(config_dir);
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
