import { useEffect, useState } from "react";
import { api, errText, type ProjectMeta, type Settings } from "../api";

type Props = {
  settings: Settings;
  onSaved: (s: Settings) => void;
  onError: (m: string) => void;
};

const PROVIDERS = [
  ["openai", "OpenAI — Luna default, Terra if needed"],
  ["anthropic", "Anthropic — Claude"],
  ["deepseek", "DeepSeek — cheapest (China-hosted)"],
] as const;

const MODELS: Record<string, [string, string][]> = {
  openai: [
    ["gpt-5.6-luna", "Luna — default ($0.20/$1.20 per Mtok)"],
    ["gpt-5.6-terra", "Terra — more power if needed ($2/$12)"],
  ],
  anthropic: [
    ["claude-haiku-4-5", "Haiku 4.5 — thin/fast ($1/$5)"],
    ["claude-sonnet-5", "Sonnet 5 ($3/$15)"],
    ["claude-opus-5", "Opus 5 ($5/$25)"],
  ],
  deepseek: [
    ["deepseek-v4-flash", "V4 Flash — cheapest ($0.14/$0.28)"],
    ["deepseek-v4-pro", "V4 Pro ($0.44/$0.87)"],
  ],
};

function defaultModelFor(provider: string): string {
  return MODELS[provider]?.[0]?.[0] ?? "gpt-5.6-luna";
}

function keyField(
  provider: Settings["provider"]
): "deepseek_api_key" | "openai_api_key" | "anthropic_api_key" {
  switch (provider) {
    case "deepseek":
      return "deepseek_api_key";
    case "anthropic":
      return "anthropic_api_key";
    default:
      return "openai_api_key";
  }
}

function keyPlaceholder(provider: string): string {
  switch (provider) {
    case "deepseek":
      return "sk-…";
    case "anthropic":
      return "sk-ant-…";
    default:
      return "sk-…";
  }
}

function keyEnvHint(provider: string): string {
  switch (provider) {
    case "deepseek":
      return "DEEPSEEK_API_KEY";
    case "anthropic":
      return "ANTHROPIC_API_KEY";
    default:
      return "OPENAI_API_KEY";
  }
}

export default function SettingsView({ settings, onSaved, onError }: Props) {
  const [draft, setDraft] = useState<Settings>(settings);
  const [saved, setSaved] = useState(false);
  const [glossary, setGlossary] = useState("");
  const [profile, setProfile] = useState("");
  const [projects, setProjects] = useState<ProjectMeta[]>([]);

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api.getGlossary().then(setGlossary).catch(() => {});
    api.getProfile().then(setProfile).catch(() => {});
    api.getProjectsConfig().then(setProjects).catch(() => {});
  }, []);

  function set<K extends keyof Settings>(k: K, v: Settings[K]) {
    setDraft({ ...draft, [k]: v });
    setSaved(false);
  }

  async function save() {
    try {
      const s = await api.saveSettings(draft);
      await api.saveGlossary(glossary);
      await api.saveProfile(profile);
      await api.saveProjectsConfig(projects);
      onSaved(s);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      onError(errText(e));
    }
  }

  function updateProject(i: number, patch: Partial<ProjectMeta>) {
    setProjects(projects.map((p, j) => (j === i ? { ...p, ...patch } : p)));
  }

  return (
    <div className="content settings">
      <section>
        <h3>Vault</h3>
        <label>
          <span>Folder</span>
          <input
            className="mono"
            value={draft.vault_path}
            onChange={(e) => set("vault_path", e.target.value)}
          />
        </label>
        <p className="dim tiny">
          Plain Markdown. Point Obsidian at this same folder if you want graph view and mobile
          sync. Nothing here is secret, so it is safe to put in its own private git repo.
        </p>
      </section>

      <section>
        <h3>Capture</h3>
        <label>
          <span>Hotkey</span>
          <input
            className="mono"
            value={draft.capture_hotkey}
            onChange={(e) => set("capture_hotkey", e.target.value)}
            placeholder="CmdOrControl+Shift+Space"
          />
        </label>
        <p className="dim tiny">
          Applied on save. If the combination is already taken by something else, saving will tell
          you.
        </p>
      </section>

      <section>
        <h3>Keyboard</h3>
        <p className="dim tiny">
          The capture hotkey above is global. These work inside the app, except while you are typing
          in an editor or input.
        </p>
        <ul className="dim tiny shortcut-list">
          <li>
            <kbd>Ctrl</kbd>+<kbd>K</kbd> — Quick switcher / command palette. Works everywhere,
            including inside an editor.
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>H</kbd> — Home
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>T</kbd> — Today
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>I</kbd> — Inbox
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>D</kbd> — Days
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>J</kbd> — Projects
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>F</kbd> — Search
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>A</kbd> — Ask
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>E</kbd> — New entry
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> — Process pending items in the current
            view (Inbox processes everything; Today and Days process that day)
          </li>
          <li>
            <kbd>Ctrl</kbd>+<kbd>S</kbd> — Save the open editor
          </li>
        </ul>
      </section>

      <section>
        <h3>Date & time</h3>
        <label>
          <span>Date format</span>
          <select
            value={draft.date_format || "DD/MM/YYYY"}
            onChange={(e) => set("date_format", e.target.value)}
          >
            <option value="DD/MM/YYYY">DD/MM/YYYY (06/08/2026)</option>
            <option value="MM/DD/YYYY">MM/DD/YYYY (08/06/2026)</option>
            <option value="YYYY-MM-DD">YYYY-MM-DD (2026-08-06)</option>
          </select>
        </label>
        <label>
          <span>Time format</span>
          <select
            value={draft.time_format || "24h"}
            onChange={(e) => set("time_format", e.target.value)}
          >
            <option value="24h">24-hour (14:30)</option>
            <option value="12h">12-hour (2:30 PM)</option>
          </select>
        </label>
        <p className="dim tiny">
          Used in the app UI and for new timestamps written into notes. File names and capture IDs
          stay ISO (<span className="mono">YYYY-MM-DD</span>) so the vault stays portable.
        </p>
      </section>

      <section>
        <h3>Model</h3>
        <label>
          <span>Provider</span>
          <select
            value={draft.provider}
            onChange={(e) => {
              const provider = e.target.value as Settings["provider"];
              const models = MODELS[provider] ?? [];
              const modelOk = models.some(([id]) => id === draft.model);
              setDraft({
                ...draft,
                provider,
                model: modelOk ? draft.model : defaultModelFor(provider),
              });
              setSaved(false);
            }}
          >
            {PROVIDERS.map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>API key</span>
          <input
            type="password"
            className="mono"
            value={draft[keyField(draft.provider)]}
            placeholder={keyPlaceholder(draft.provider)}
            onChange={(e) => set(keyField(draft.provider), e.target.value)}
          />
        </label>
        <p className="dim tiny">
          Stored in the app config directory, never in the vault. Each provider keeps its own key so
          switching does not wipe the others. Leave blank to fall back to{" "}
          <span className="mono">{keyEnvHint(draft.provider)}</span>.
        </p>
        <label>
          <span>Model</span>
          <select value={draft.model} onChange={(e) => set("model", e.target.value)}>
            {(MODELS[draft.provider] ?? []).map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <p className="dim tiny">
          Default is GPT-5.6 Luna — cheap enough for daily triage without sending journal text to
          China-hosted infra. Bump to Terra if routing quality is thin; DeepSeek stays available if
          you ever want the absolute cheapest option.
        </p>
        <label>
          <span>Effort</span>
          <select
            value={draft.effort}
            onChange={(e) => set("effort", e.target.value as Settings["effort"])}
          >
            {["low", "medium", "high", "xhigh", "max"].map((e) => (
              <option key={e} value={e}>
                {e}
              </option>
            ))}
          </select>
        </label>
        <p className="dim tiny">
          Used by OpenAI (reasoning effort) and Anthropic. Ignored on DeepSeek, where triage runs
          with thinking disabled.
        </p>
        <label>
          <span>Context days</span>
          <input
            type="number"
            min={0}
            max={14}
            value={draft.context_days}
            onChange={(e) => set("context_days", Number(e.target.value))}
          />
        </label>
        <p className="dim tiny">
          Reserved for continuity context on future passes. Triage currently uses profile + known
          projects only.
        </p>
      </section>

      <section>
        <h3>Profile</h3>
        <p className="dim tiny">
          Durable facts about you, sent with every triage pass so you never have to re-explain
          yourself. Keep it short and factual.
        </p>
        <textarea
          className="mono tall"
          value={profile}
          onChange={(e) => setProfile(e.target.value)}
          spellCheck={false}
        />
      </section>

      <section>
        <h3>Glossary</h3>
        <p className="dim tiny">
          One term per line. Dictation mangles proper nouns constantly, and this is what lets the
          model repair them. Worth more than any other setting on this page.
        </p>
        <textarea
          className="mono tall"
          value={glossary}
          onChange={(e) => setGlossary(e.target.value)}
          spellCheck={false}
        />
      </section>

      <section>
        <h3>Projects & areas</h3>
        <p className="dim tiny">
          Learned automatically when the inbox is processed. Projects have an end state; areas do
          not. Scope (personal/work) is independent of kind. Aliases make "that bike thing" route
          correctly.
        </p>
        {projects.length === 0 && <p className="dim">None yet.</p>}
        {projects.map((p, i) => (
          <div key={p.slug} className="projedit">
            <div className="mono tiny dim">{p.slug}</div>
            <input
              value={p.name}
              onChange={(e) => updateProject(i, { name: e.target.value })}
              placeholder="Display name"
            />
            <div className="row gap">
              <select
                value={p.kind || "project"}
                onChange={(e) => updateProject(i, { kind: e.target.value })}
              >
                <option value="project">project</option>
                <option value="area">area</option>
              </select>
              <select
                value={p.scope || "work"}
                onChange={(e) => updateProject(i, { scope: e.target.value })}
              >
                <option value="work">work</option>
                <option value="personal">personal</option>
              </select>
            </div>
            <input
              value={p.aliases.join(", ")}
              onChange={(e) =>
                updateProject(i, {
                  aliases: e.target.value
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean),
                })
              }
              placeholder="aliases, comma separated"
            />
            <input
              value={p.description}
              onChange={(e) => updateProject(i, { description: e.target.value })}
              placeholder="one line of context for the model"
            />
          </div>
        ))}
      </section>

      <div className="savebar">
        <button className="btn primary" onClick={save}>
          Save settings
        </button>
        {saved && <span className="good">Saved</span>}
      </div>
    </div>
  );
}
