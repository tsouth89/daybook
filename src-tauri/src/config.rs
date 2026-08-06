use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App settings. Stored as JSON next to the app's config dir, NOT in the vault,
/// so the vault stays portable and contains no secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub vault_path: String,
    /// deepseek | openai | anthropic
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    /// DeepSeek API key (platform.deepseek.com).
    #[serde(default)]
    pub deepseek_api_key: String,
    /// OpenAI API key (Luna / Terra).
    #[serde(default)]
    pub openai_api_key: String,
    /// Anthropic API key.
    #[serde(default)]
    pub anthropic_api_key: String,
    /// Legacy single-key field. Migrated into anthropic_api_key on load.
    #[serde(default)]
    pub api_key: String,
    /// Provider-specific effort/thinking hint: low | medium | high | xhigh | max
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default = "default_hotkey")]
    pub capture_hotkey: String,
    /// How many previous day-summaries to feed in as continuity context.
    #[serde(default = "default_context_days")]
    pub context_days: usize,
}

fn default_provider() -> String {
    "deepseek".into()
}
fn default_model() -> String {
    "deepseek-v4-flash".into()
}
fn default_effort() -> String {
    // Triage is classification + light rewrite, not deep reasoning.
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
            provider: default_provider(),
            model: default_model(),
            deepseek_api_key: String::new(),
            openai_api_key: String::new(),
            anthropic_api_key: String::new(),
            api_key: String::new(),
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

    pub fn normalized_provider(&self) -> &str {
        match self.provider.to_lowercase().as_str() {
            "openai" => "openai",
            "anthropic" => "anthropic",
            _ => "deepseek",
        }
    }

    /// Key for the active provider. Falls back to the matching env var.
    pub fn resolved_api_key(&self) -> String {
        let (stored, env_name) = match self.normalized_provider() {
            "openai" => (self.openai_api_key.trim(), "OPENAI_API_KEY"),
            "anthropic" => {
                let k = if !self.anthropic_api_key.trim().is_empty() {
                    self.anthropic_api_key.trim()
                } else {
                    // Legacy field from before multi-provider support.
                    self.api_key.trim()
                };
                (k, "ANTHROPIC_API_KEY")
            }
            _ => (self.deepseek_api_key.trim(), "DEEPSEEK_API_KEY"),
        };
        if !stored.is_empty() {
            return stored.to_string();
        }
        std::env::var(env_name).unwrap_or_default()
    }

    /// Pull legacy `api_key` into `anthropic_api_key` so old settings keep working.
    /// Also reconcile provider/model when settings predate multi-provider support.
    pub fn migrate_keys(&mut self) {
        if self.anthropic_api_key.trim().is_empty() && !self.api_key.trim().is_empty() {
            self.anthropic_api_key = self.api_key.trim().to_string();
        }
        if self.provider.trim().is_empty() {
            self.provider = default_provider();
        }
        if self.model.trim().is_empty() {
            self.model = default_model();
        }

        // Pre-provider installs saved claude-* with no provider field. Serde now
        // defaults provider to deepseek, which would call DeepSeek with a Claude
        // model id. Reconcile:
        // - If they already have an Anthropic key, keep them on Anthropic.
        // - Otherwise move them onto Flash (the new cheap default).
        if self.normalized_provider() == "deepseek" && self.model.starts_with("claude") {
            if !self.anthropic_api_key.trim().is_empty() {
                self.provider = "anthropic".into();
            } else {
                self.model = default_model();
            }
        }
        if self.normalized_provider() == "openai" && !self.model.starts_with("gpt-") {
            self.model = "gpt-5.6-luna".into();
        }
        if self.normalized_provider() == "anthropic" && !self.model.starts_with("claude") {
            self.model = "claude-haiku-4-5".into();
        }
        if self.normalized_provider() == "deepseek" && !self.model.starts_with("deepseek") {
            self.model = default_model();
        }
    }
}

pub fn settings_path(config_dir: &PathBuf) -> PathBuf {
    config_dir.join("settings.json")
}

pub fn load(config_dir: &PathBuf) -> Settings {
    let path = settings_path(config_dir);
    let mut settings = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default();
    settings.migrate_keys();
    settings
}

pub fn save(config_dir: &PathBuf, settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(config_dir).context("creating config dir")?;
    let path = settings_path(config_dir);
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
