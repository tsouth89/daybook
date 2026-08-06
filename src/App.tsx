import { useCallback, useEffect, useState } from "react";
import { api, errText, type DayEntry, type Settings } from "./api";
import DaysView from "./views/DaysView";
import HistoryView from "./views/HistoryView";
import IdeasView from "./views/IdeasView";
import InboxView from "./views/InboxView";
import PersonalView from "./views/PersonalView";
import ProjectsView from "./views/ProjectsView";
import SearchView from "./views/SearchView";
import SettingsView from "./views/SettingsView";
import TasksView from "./views/TasksView";

type Tab =
  | "inbox"
  | "days"
  | "personal"
  | "projects"
  | "tasks"
  | "ideas"
  | "history"
  | "search"
  | "settings";

function hasProviderKey(s: Settings): boolean {
  switch (s.provider) {
    case "deepseek":
      return !!s.deepseek_api_key.trim();
    case "anthropic":
      return !!(s.anthropic_api_key.trim() || s.api_key.trim());
    default:
      return !!s.openai_api_key.trim();
  }
}

export default function App() {
  const [tab, setTab] = useState<Tab>("inbox");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [days, setDays] = useState<DayEntry[]>([]);
  const [inboxCount, setInboxCount] = useState(0);
  const [banner, setBanner] = useState<string | null>(null);

  const refreshDays = useCallback(async () => {
    try {
      setDays(await api.listDays());
    } catch (e) {
      setBanner(errText(e));
    }
  }, []);

  const refreshInbox = useCallback(async () => {
    try {
      const items = await api.listInbox();
      setInboxCount(items.length);
    } catch {
      /* vault may not exist yet */
    }
  }, []);

  useEffect(() => {
    api.getSettings().then(setSettings).catch((e) => setBanner(errText(e)));
    refreshDays();
    refreshInbox();
  }, [refreshDays, refreshInbox]);

  useEffect(() => {
    const onFocus = () => {
      refreshDays();
      refreshInbox();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refreshDays, refreshInbox]);

  const needsKey = settings && !hasProviderKey(settings);
  const vaultPath = settings?.vault_path ?? "";

  return (
    <div className="shell">
      <nav className="sidebar">
        <div className="brand">Daybook</div>
        <button className="btn primary capture-btn" onClick={() => api.showCapture()}>
          New entry
        </button>
        {(
          [
            ["inbox", inboxCount ? `Inbox (${inboxCount})` : "Inbox"],
            ["days", "Days"],
            ["personal", "Personal"],
            ["projects", "Projects"],
            ["tasks", "Tasks"],
            ["ideas", "Ideas"],
            ["history", "History"],
            ["search", "Search"],
            ["settings", "Settings"],
          ] as [Tab, string][]
        ).map(([k, label]) => (
          <button
            key={k}
            className={`navitem ${tab === k ? "active" : ""}`}
            onClick={() => setTab(k)}
          >
            {label}
          </button>
        ))}
        <div className="spacer" />
        {settings && (
          <div className="vaultbox">
            <div className="dim tiny">Vault</div>
            <div className="tiny mono wrap">{settings.vault_path}</div>
            <button className="btn tiny-btn" onClick={() => api.revealVault()}>
              Open folder
            </button>
          </div>
        )}
      </nav>

      <main className="main">
        {banner && (
          <div className="banner bad" onClick={() => setBanner(null)}>
            {banner} <span className="dim">(click to dismiss)</span>
          </div>
        )}
        {needsKey && tab !== "settings" && (
          <div className="banner warn">
            No API key set for the current provider, so captures stay in the inbox until you add one.{" "}
            <button className="linkbtn" onClick={() => setTab("settings")}>
              Add one in Settings
            </button>
          </div>
        )}

        {tab === "inbox" && (
          <InboxView
            vaultPath={vaultPath}
            onChanged={() => {
              refreshDays();
              refreshInbox();
            }}
            onError={setBanner}
          />
        )}
        {tab === "days" && (
          <DaysView
            days={days}
            vaultPath={vaultPath}
            onChanged={refreshDays}
            onError={setBanner}
          />
        )}
        {tab === "personal" && (
          <PersonalView vaultPath={vaultPath} onError={setBanner} />
        )}
        {tab === "projects" && (
          <ProjectsView vaultPath={vaultPath} onError={setBanner} />
        )}
        {tab === "tasks" && <TasksView onError={setBanner} />}
        {tab === "ideas" && (
          <IdeasView vaultPath={vaultPath} onError={setBanner} />
        )}
        {tab === "history" && (
          <HistoryView vaultPath={vaultPath} onError={setBanner} />
        )}
        {tab === "search" && <SearchView onError={setBanner} />}
        {tab === "settings" && settings && (
          <SettingsView
            settings={settings}
            onSaved={(s) => {
              setSettings(s);
              refreshDays();
              refreshInbox();
            }}
            onError={setBanner}
          />
        )}
      </main>
    </div>
  );
}
