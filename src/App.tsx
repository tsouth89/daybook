import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, errText, type DayEntry, type Settings } from "./api";
import ConfirmDialog from "./ConfirmDialog";
import { ContextMenu, useContextMenu } from "./ContextMenu";
import { FormatProvider } from "./FormatContext";
import { NavProvider, type NavTarget } from "./nav";
import { ViewHostProvider, type ViewHandlers } from "./viewhost";
import Palette, { type Command } from "./Palette";
import DaysView from "./views/DaysView";
import HistoryView from "./views/HistoryView";
import IdeasView from "./views/IdeasView";
import InboxView from "./views/InboxView";
import PersonalView from "./views/PersonalView";
import ProjectsView from "./views/ProjectsView";
import SearchView from "./views/SearchView";
import SettingsView from "./views/SettingsView";
import TasksView from "./views/TasksView";
import TodayView from "./views/TodayView";
import HomeView from "./views/HomeView";
import AskView from "./views/AskView";

type Tab =
  | "home"
  | "today"
  | "inbox"
  | "days"
  | "personal"
  | "projects"
  | "tasks"
  | "ideas"
  | "history"
  | "ask"
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

function isTypingTarget(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  const tag = el.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  if (el.isContentEditable) return true;
  if (el.closest(".cm-editor")) return true;
  return false;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("home");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [days, setDays] = useState<DayEntry[]>([]);
  const [inboxCount, setInboxCount] = useState(0);
  const [todayPending, setTodayPending] = useState(0);
  const [todayIso, setTodayIso] = useState<string | null>(null);
  const [banner, setBanner] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [focusDay, setFocusDay] = useState<string | null>(null);
  const [focusDayPane, setFocusDayPane] = useState<"note" | "raw" | null>(null);
  const [focusEntity, setFocusEntity] = useState<string | null>(null);
  /** Navigation held back by an unsaved editor, replayed if the user confirms. */
  const [blockedNav, setBlockedNav] = useState<(() => void) | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const handlers = useRef<ViewHandlers>({});
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
      // Re-reading today's date here also handles the app being left open past midnight.
      const [items, today] = await Promise.all([api.listInbox(), api.todayDate()]);
      setInboxCount(items.length);
      setTodayIso(today);
      setTodayPending(items.filter((i) => i.date === today).length);
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

  function flash(msg: string) {
    setNotice(msg);
  }

  /** Run `action` unless the current view has an unsaved editor open. */
  const guard = useCallback((action: () => void) => {
    if (handlers.current.isDirty?.()) {
      setBlockedNav(() => action);
      return;
    }
    action();
  }, []);

  const goTab = useCallback(
    (next: Tab) => {
      guard(() => setTab(next));
    },
    [guard]
  );

  const navigate = useCallback(
    (target: NavTarget) => {
      guard(() => {
        switch (target.type) {
          case "tab":
            setTab(target.tab as Tab);
            break;
          case "day":
            setFocusDay(target.date);
            setFocusDayPane(target.pane ?? "note");
            setTab("days");
            break;
          case "entity":
            setFocusEntity(`${target.kind}:${target.slug}`);
            setTab("projects");
            break;
        }
      });
    },
    [guard]
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // The palette is reachable from anywhere, including inside an editor.
      if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
        const key = e.key.toLowerCase();
        if (key === "k" || key === "o") {
          e.preventDefault();
          setPaletteOpen((v) => !v);
          return;
        }
      }
      if (!(e.ctrlKey || e.metaKey) || !e.shiftKey || e.altKey) return;
      if (isTypingTarget(e.target)) return;
      const k = e.key.toLowerCase();

      // Ctrl+Shift+P processes whatever the current view has pending.
      if (k === "p") {
        e.preventDefault();
        if (handlers.current.process) handlers.current.process();
        else flash("Nothing to process here");
        return;
      }
      if (k === "e") {
        e.preventDefault();
        void api.showCapture();
        return;
      }
      const map: Record<string, Tab> = {
        h: "home",
        t: "today",
        i: "inbox",
        d: "days",
        j: "projects",
        f: "search",
        a: "ask",
      };
      const next = map[k];
      if (next) {
        e.preventDefault();
        goTab(next);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [goTab]);

  const paletteCommands = useMemo<Command[]>(() => {
    const tabs: [Tab, string][] = [
      ["home", "Home"],
      ["today", "Today"],
      ["inbox", "Inbox"],
      ["days", "Days"],
      ["personal", "Personal"],
      ["projects", "Projects"],
      ["tasks", "Tasks"],
      ["ideas", "Ideas"],
      ["history", "History"],
      ["ask", "Ask"],
      ["search", "Search"],
      ["settings", "Settings"],
    ];
    return [
      ...tabs.map(([t, label]) => ({
        id: `go-${t}`,
        label: `Go to ${label}`,
        run: () => goTab(t),
      })),
      {
        id: "capture",
        label: "New capture",
        hint: "Ctrl+Shift+E",
        run: () => void api.showCapture(),
      },
      {
        id: "process",
        label: "Process pending in this view",
        hint: "Ctrl+Shift+P",
        run: () => handlers.current.process?.(),
      },
      {
        id: "reveal",
        label: "Open vault folder",
        run: () => void api.revealVault().catch((e) => setBanner(errText(e))),
      },
      {
        id: "rescan",
        label: "Rescan vault for entries",
        run: () =>
          void api
            .rebuildEntryIndex()
            .then((r) => flash(`Recovered ${r.recovered} · kept ${r.kept}`))
            .catch((e) => setBanner(errText(e))),
      },
    ];
  }, [goTab]);

  const needsKey = settings && !hasProviderKey(settings);
  const vaultPath = settings?.vault_path ?? "";

  const onChanged = useCallback(() => {
    refreshDays();
    refreshInbox();
  }, [refreshDays, refreshInbox]);

  return (
    <FormatProvider
      dateFormat={settings?.date_format}
      timeFormat={settings?.time_format}
    >
      <NavProvider navigate={navigate}>
        <ViewHostProvider handlers={handlers}>
          <div className="shell">
            <nav
              className="sidebar"
              onContextMenu={(e) => {
                if ((e.target as HTMLElement).closest("button")) return;
                menu.open(e, [
                  {
                    label: "New entry",
                    shortcut: "⌃⇧Space",
                    onClick: () => void api.showCapture(),
                  },
                  { kind: "sep" },
                  { label: "Go to Home", shortcut: "⌃⇧H", onClick: () => goTab("home") },
                  { label: "Go to Today", shortcut: "⌃⇧T", onClick: () => goTab("today") },
                  { label: "Go to Inbox", shortcut: "⌃⇧I", onClick: () => goTab("inbox") },
                  { label: "Go to Days", shortcut: "⌃⇧D", onClick: () => goTab("days") },
                  { label: "Go to Projects", shortcut: "⌃⇧J", onClick: () => goTab("projects") },
                  { kind: "sep" },
                  {
                    label: "Open vault folder",
                    onClick: () => void api.revealVault().catch((err) => setBanner(errText(err))),
                  },
                  { label: "Settings", onClick: () => goTab("settings") },
                ]);
              }}
            >
              <div className="brand">Daybook</div>
              <button className="btn primary capture-btn" onClick={() => api.showCapture()}>
                New entry
              </button>
              {(
                [
                  ["home", "Home"],
                  ["today", todayPending ? `Today (${todayPending})` : "Today"],
                  ["inbox", inboxCount ? `Inbox (${inboxCount})` : "Inbox"],
                  ["days", "Days"],
                  ["personal", "Personal"],
                  ["projects", "Projects"],
                  ["tasks", "Tasks"],
                  ["ideas", "Ideas"],
                  ["history", "History"],
                  ["ask", "Ask"],
                  ["search", "Search"],
                  ["settings", "Settings"],
                ] as [Tab, string][]
              ).map(([k, label]) => (
                <button
                  key={k}
                  className={`navitem ${tab === k ? "active" : ""}`}
                  onClick={() => goTab(k)}
                  onContextMenu={(e) =>
                    menu.open(e, [
                      {
                        label: `Open ${label.replace(/\s*\(.*\)/, "")}`,
                        onClick: () => goTab(k),
                      },
                      { kind: "sep" },
                      { label: "New entry", onClick: () => void api.showCapture() },
                      {
                        label: "Open vault folder",
                        onClick: () =>
                          void api.revealVault().catch((err) => setBanner(errText(err))),
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
                        onClick: () =>
                          void api.revealVault().catch((err) => setBanner(errText(err))),
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
                  No API key set for the current provider, so captures stay in the inbox until you
                  add one.{" "}
                  <button className="linkbtn" onClick={() => goTab("settings")}>
                    Add one in Settings
                  </button>
                </div>
              )}

              {tab === "home" &&
                (todayIso ? (
                  <HomeView
                    date={todayIso}
                    onChanged={onChanged}
                    onError={setBanner}
                    onNotice={flash}
                  />
                ) : (
                  <div className="empty">
                    <h2>Loading…</h2>
                  </div>
                ))}
              {tab === "today" &&
                (todayIso ? (
                  <TodayView
                    date={todayIso}
                    vaultPath={vaultPath}
                    onChanged={onChanged}
                    onError={setBanner}
                    onNotice={flash}
                  />
                ) : (
                  <div className="empty">
                    <h2>Loading today…</h2>
                  </div>
                ))}
              {tab === "inbox" && (
                <InboxView
                  vaultPath={vaultPath}
                  onChanged={onChanged}
                  onError={setBanner}
                  onNotice={flash}
                />
              )}
              {tab === "days" && (
                <DaysView
                  days={days}
                  vaultPath={vaultPath}
                  focusDate={focusDay}
                  focusPane={focusDayPane}
                  onFocusConsumed={() => {
                    setFocusDay(null);
                    setFocusDayPane(null);
                  }}
                  onChanged={onChanged}
                  onError={setBanner}
                  onNotice={flash}
                />
              )}
              {tab === "personal" && (
                <PersonalView vaultPath={vaultPath} onError={setBanner} onNotice={flash} />
              )}
              {tab === "projects" && (
                <ProjectsView
                  vaultPath={vaultPath}
                  focusKey={focusEntity}
                  onFocusConsumed={() => setFocusEntity(null)}
                  onError={setBanner}
                  onNotice={flash}
                />
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
                    setFocusDayPane("note");
                    setTab("days");
                  }}
                />
              )}
              {tab === "ask" && <AskView vaultPath={vaultPath} onError={setBanner} />}
              {tab === "search" && <SearchView onError={setBanner} onNotice={flash} />}
              {tab === "settings" && settings && (
                <SettingsView
                  settings={settings}
                  onSaved={(s) => {
                    setSettings(s);
                    onChanged();
                  }}
                  onError={setBanner}
                />
              )}
            </main>
            <ContextMenu {...menu.menuProps} />
            <Palette
              open={paletteOpen}
              onClose={() => setPaletteOpen(false)}
              navigate={navigate}
              commands={paletteCommands}
            />
            <ConfirmDialog
              open={!!blockedNav}
              title="Leave without saving?"
              body="This view has an open editor with unsaved changes. Leaving now discards them."
              confirmLabel="Discard changes"
              danger
              onCancel={() => setBlockedNav(null)}
              onConfirm={() => {
                const go = blockedNav;
                setBlockedNav(null);
                go?.();
              }}
            />
          </div>
        </ViewHostProvider>
      </NavProvider>
    </FormatProvider>
  );
}
