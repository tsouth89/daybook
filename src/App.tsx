import { useCallback, useEffect, useState } from "react";
import { api, errText, type DayEntry, type Settings } from "./api";
import { ContextMenu, useContextMenu } from "./ContextMenu";
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
  const [notice, setNotice] = useState<string | null>(null);
  const [focusDay, setFocusDay] = useState<string | null>(null);
  const menu = useContextMenu();

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

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 2200);
    return () => clearTimeout(t);
  }, [notice]);

  const needsKey = settings && !hasProviderKey(settings);
  const vaultPath = settings?.vault_path ?? "";

  function flash(msg: string) {
    setNotice(msg);
  }

  return (
    <div className="shell">
      <nav
        className="sidebar"
        onContextMenu={(e) => {
          if ((e.target as HTMLElement).closest("button")) return;
          menu.open(e, [
            { label: "New entry", shortcut: "⌃⇧Space", onClick: () => void api.showCapture() },
            { kind: "sep" },
            { label: "Go to Inbox", onClick: () => setTab("inbox") },
            { label: "Go to Days", onClick: () => setTab("days") },
            { label: "Go to Projects", onClick: () => setTab("projects") },
            { kind: "sep" },
            {
              label: "Open vault folder",
              onClick: () => void api.revealVault().catch((err) => setBanner(errText(err))),
            },
            { label: "Settings", onClick: () => setTab("settings") },
          ]);
        }}
      >
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
            onContextMenu={(e) =>
              menu.open(e, [
                { label: `Open ${label.replace(/\s*\(.*\)/, "")}`, onClick: () => setTab(k) },
                { kind: "sep" },
                {
                  label: "New entry",
                  onClick: () => void api.showCapture(),
                },
                {
                  label: "Open vault folder",
                  onClick: () => void api.revealVault().catch((err) => setBanner(errText(err))),
                },
              ])
            }
          >
            {label}
          </button>
        ))}
        <div className="spacer" />
        {settings && (
          <div
            className="vaultbox"
            onContextMenu={(e) =>
              menu.open(e, [
                {
                  label: "Copy vault path",
                  onClick: () => {
                    void navigator.clipboard.writeText(settings.vault_path);
                    flash("Copied vault path");
                  },
                },
                {
                  label: "Open vault folder",
                  onClick: () => void api.revealVault().catch((err) => setBanner(errText(err))),
                },
              ])
            }
          >
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
        {notice && !banner && <div className="banner ok">{notice}</div>}
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
            onNotice={flash}
          />
        )}
        {tab === "days" && (
          <DaysView
            days={days}
            vaultPath={vaultPath}
            focusDate={focusDay}
            onFocusConsumed={() => setFocusDay(null)}
            onChanged={refreshDays}
            onError={setBanner}
            onNotice={flash}
          />
        )}
        {tab === "personal" && (
          <PersonalView vaultPath={vaultPath} onError={setBanner} onNotice={flash} />
        )}
        {tab === "projects" && (
          <ProjectsView vaultPath={vaultPath} onError={setBanner} onNotice={flash} />
        )}
        {tab === "tasks" && <TasksView onError={setBanner} onNotice={flash} />}
        {tab === "ideas" && (
          <IdeasView vaultPath={vaultPath} onError={setBanner} onNotice={flash} />
        )}
        {tab === "history" && (
          <HistoryView
            vaultPath={vaultPath}
            onError={setBanner}
            onNotice={flash}
            onOpenDay={(date) => {
              setFocusDay(date);
              setTab("days");
            }}
          />
        )}
        {tab === "search" && <SearchView onError={setBanner} onNotice={flash} />}
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
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
