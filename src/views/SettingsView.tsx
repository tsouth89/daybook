import { useEffect, useState } from "react";
import { api, errText, type ProjectMeta, type Settings } from "../api";

type Props = {
  settings: Settings;
  onSaved: (s: Settings) => void;
  onError: (m: string) => void;
};

const MODELS = [
  ["claude-opus-5", "Opus 5 — best quality ($5/$25 per Mtok)"],
  ["claude-sonnet-5", "Sonnet 5 — cheaper, still strong ($3/$15)"],
  ["claude-haiku-4-5", "Haiku 4.5 — fastest, thinnest output ($1/$5)"],
];

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
        <h3>Model</h3>
        <label>
          <span>API key</span>
          <input
            type="password"
            className="mono"
            value={draft.api_key}
            placeholder="sk-ant-…"
            onChange={(e) => set("api_key", e.target.value)}
          />
        </label>
        <p className="dim tiny">
          Stored in the app's own config directory, never in the vault. Leave blank to fall back to
          the ANTHROPIC_API_KEY environment variable.
        </p>
        <label>
          <span>Model</span>
          <select value={draft.model} onChange={(e) => set("model", e.target.value)}>
            {MODELS.map(([id, label]) => (
              <option key={id} value={id}>
                {label}
              </option>
            ))}
          </select>
        </label>
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
          Summarising a day is not an intelligence-heavy task, so medium is the default. Raise it if
          summaries come out thin or projects get mis-routed.
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
          How many previous day summaries get sent along so the model can resolve "that thing from
          yesterday".
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
