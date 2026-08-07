import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  errText,
  type Entry,
  type InboxProcessResult,
  type ProjectEntry,
} from "../api";
import { ContextMenu, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import ProcessResult from "../ProcessResult";
import { useNavigate } from "../nav";
import { useViewHandlers } from "../viewhost";

type Props = {
  /** ISO date for today, from the backend clock. */
  date: string;
  onChanged: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

/** Group entries by owning project, unowned last. */
function byProject(entries: Entry[]): { slug: string; name: string; entries: Entry[] }[] {
  const groups = new Map<string, { slug: string; name: string; entries: Entry[] }>();
  for (const e of entries) {
    const key = e.slug || "";
    const g = groups.get(key);
    if (g) {
      g.entries.push(e);
      if (!g.name && e.name) g.name = e.name;
    } else {
      groups.set(key, { slug: key, name: e.name || key, entries: [e] });
    }
  }
  return [...groups.values()].sort((a, b) => {
    if (!a.slug) return 1;
    if (!b.slug) return -1;
    return b.entries.length - a.entries.length;
  });
}

/**
 * The hub. Everything here is a query over the item layer rather than a file
 * being read — which is the whole point of keeping triage's properties.
 */
export default function HomeView({ date, onChanged, onError, onNotice }: Props) {
  const [openLoops, setOpenLoops] = useState<Entry[]>([]);
  const [tasks, setTasks] = useState<Entry[]>([]);
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [recent, setRecent] = useState<Entry[]>([]);
  const [pending, setPending] = useState(0);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const menu = useContextMenu();
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    try {
      const [loops, open, projs, latest, inbox] = await Promise.all([
        api.queryEntries({ open_only: true, limit: 60 }),
        api.queryEntries({ kind: "task", undone_only: true, limit: 100 }),
        api.listProjects(),
        api.queryEntries({ limit: 12 }),
        api.listInbox(),
      ]);
      setOpenLoops(loops);
      setTasks(open);
      setProjects(projs);
      setRecent(latest);
      setPending(inbox.filter((i) => i.date === date).length);
      return loops.length + open.length + latest.length;
    } catch (e) {
      onError(errText(e));
      return 0;
    }
  }, [date, onError]);

  useEffect(() => {
    load();
  }, [load]);

  // First run on an existing vault: the index is empty but the markdown is
  // full. Recovering costs nothing, so just do it rather than asking.
  useEffect(() => {
    if (scanned || scanning) return;
    let cancelled = false;
    (async () => {
      const n = await load();
      if (cancelled || n > 0) {
        setScanned(true);
        return;
      }
      setScanning(true);
      try {
        const report = await api.rebuildEntryIndex();
        if (!cancelled && report.recovered > 0) {
          onNotice?.(`Recovered ${report.recovered} entries from your vault`);
          await load();
        }
      } catch (e) {
        onError(errText(e));
      } finally {
        if (!cancelled) {
          setScanning(false);
          setScanned(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [scanned, scanning, load, onError, onNotice]);

  useEffect(() => {
    const onFocus = () => load();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load]);

  const process = useCallback(async () => {
    if (busy || pending === 0) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processDay(date);
      setResult(r);
      await load();
      onChanged();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }, [busy, pending, date, load, onChanged, onError]);

  useViewHandlers({ process: () => void process() });

  async function toggleTask(entry: Entry) {
    try {
      await api.setTaskDone(entry.id, !entry.done);
      setTasks((prev) => prev.filter((t) => t.id !== entry.id));
      onNotice?.(entry.done ? "Reopened" : "Done");
    } catch (e) {
      onError(errText(e));
    }
  }

  const overdue = useMemo(
    () => tasks.filter((t) => t.due && t.due < date),
    [tasks, date]
  );
  const dueToday = useMemo(() => tasks.filter((t) => t.due === date), [tasks, date]);
  const rest = useMemo(
    () => tasks.filter((t) => !t.due || t.due > date),
    [tasks, date]
  );

  const activeProjects = useMemo(() => {
    const openBySlug = new Map<string, number>();
    for (const e of openLoops) {
      if (e.slug) openBySlug.set(e.slug, (openBySlug.get(e.slug) ?? 0) + e.open.length);
    }
    const taskBySlug = new Map<string, number>();
    for (const t of tasks) {
      if (t.slug) taskBySlug.set(t.slug, (taskBySlug.get(t.slug) ?? 0) + 1);
    }
    const rank = (s: string) => (s === "active" ? 0 : s === "paused" ? 1 : 2);
    return [...projects]
      .filter((p) => p.status !== "done")
      .sort(
        (a, b) =>
          rank(a.status) - rank(b.status) ||
          (b.last_date || "").localeCompare(a.last_date || "")
      )
      .slice(0, 8)
      .map((p) => ({
        ...p,
        openCount: openBySlug.get(p.slug) ?? 0,
        taskCount: taskBySlug.get(p.slug) ?? 0,
      }));
  }, [projects, openLoops, tasks]);

  function openEntity(e: { slug: string; kind?: string }) {
    if (!e.slug) return;
    const kind = projects.find((p) => p.slug === e.slug)?.kind === "area" ? "area" : "project";
    navigate({ type: "entity", kind, slug: e.slug });
  }

  function taskRow(t: Entry, flag?: "overdue" | "today") {
    return (
      <li key={t.id} className="task-row">
        <input
          type="checkbox"
          checked={t.done}
          onChange={() => void toggleTask(t)}
          aria-label={t.title}
        />
        <span className="task-text">{t.title}</span>
        {flag === "overdue" && t.due && (
          <span className="pill bad">overdue {fmt.date(t.due)}</span>
        )}
        {flag === "today" && <span className="pill warn">today</span>}
        {!flag && t.due && <span className="dim tiny">{fmt.date(t.due)}</span>}
        {t.slug && (
          <button className="chip clickable" onClick={() => openEntity(t)}>
            {t.name || t.slug}
          </button>
        )}
      </li>
    );
  }

  const nothingYet =
    !scanning && openLoops.length === 0 && tasks.length === 0 && recent.length === 0;

  return (
    <div
      className="content home"
      onContextMenu={(e) => {
        if ((e.target as HTMLElement).closest("button, input, .ctxmenu")) return;
        menu.open(e, [
          { label: "New entry", onClick: () => void api.showCapture() },
          {
            label: `Process today (${pending})`,
            disabled: busy || pending === 0,
            onClick: () => void process(),
          },
          { kind: "sep" },
          {
            label: "Rescan vault for entries",
            disabled: scanning,
            onClick: () => {
              setScanning(true);
              api
                .rebuildEntryIndex()
                .then((r) => {
                  onNotice?.(
                    `Recovered ${r.recovered} entries · ${r.kept} kept · ${r.tasks_marked} tasks tagged`
                  );
                  return load();
                })
                .catch((err) => onError(errText(err)))
                .finally(() => setScanning(false));
            },
          },
        ]);
      }}
    >
      <div className="home-head">
        <div>
          <h1 className="home-title">{fmt.date(date)}</h1>
          <div className="dim tiny">
            {pending > 0
              ? `${pending} capture${pending === 1 ? "" : "s"} waiting`
              : "Inbox clear for today"}
          </div>
        </div>
        <div className="grow" />
        <button className="btn" onClick={() => api.showCapture()}>
          New entry
        </button>
        <button
          className="btn primary"
          onClick={() => void process()}
          disabled={busy || pending === 0}
          title="Ctrl+Shift+P"
        >
          {busy ? "Processing…" : pending > 0 ? `Process (${pending})` : "Nothing pending"}
        </button>
      </div>

      {result && <ProcessResult result={result} label="Filed" />}
      {scanning && <div className="banner">Scanning the vault for entries…</div>}

      {nothingYet ? (
        <div className="empty" style={{ padding: "24px 0" }}>
          <h2>Nothing here yet</h2>
          <p className="dim">
            Capture a thought and process it. Once entries are filed, this page becomes the
            place you actually look — open loops, outstanding tasks, and what each project is
            waiting on.
          </p>
          <p className="dim" style={{ marginTop: 16 }}>
            <button className="btn primary" onClick={() => api.showCapture()}>
              New entry
            </button>
          </p>
        </div>
      ) : (
        <div className="home-grid">
          <section className="home-card">
            <h3 className="section-label">
              Open loops {openLoops.length > 0 && `(${openLoops.length})`}
            </h3>
            {openLoops.length === 0 ? (
              <p className="dim tiny">Nothing outstanding. Enjoy it.</p>
            ) : (
              byProject(openLoops).map((g) => (
                <div key={g.slug || "_none"} className="loop-group">
                  <div className="loop-group-head">
                    {g.slug ? (
                      <button className="linkbtn" onClick={() => openEntity(g)}>
                        {g.name || g.slug}
                      </button>
                    ) : (
                      <span className="dim">Unassigned</span>
                    )}
                  </div>
                  <ul className="loop-list">
                    {g.entries.flatMap((e) =>
                      e.open.map((line, i) => (
                        <li key={`${e.id}-${i}`}>
                          <span>{line}</span>
                          <span className="dim tiny"> · {fmt.date(e.date)}</span>
                        </li>
                      ))
                    )}
                  </ul>
                </div>
              ))
            )}
          </section>

          <section className="home-card">
            <h3 className="section-label">
              Tasks {tasks.length > 0 && `(${tasks.length})`}
            </h3>
            {tasks.length === 0 ? (
              <p className="dim tiny">No open tasks.</p>
            ) : (
              <ul className="task-list">
                {overdue.map((t) => taskRow(t, "overdue"))}
                {dueToday.map((t) => taskRow(t, "today"))}
                {rest.map((t) => taskRow(t))}
              </ul>
            )}
          </section>

          <section className="home-card">
            <h3 className="section-label">Projects</h3>
            {activeProjects.length === 0 ? (
              <p className="dim tiny">No projects yet.</p>
            ) : (
              <ul className="proj-list">
                {activeProjects.map((p) => (
                  <li key={`${p.kind}:${p.slug}`}>
                    <button className="linkbtn" onClick={() => openEntity(p)}>
                      {p.name}
                    </button>
                    <span className="dim tiny">
                      {p.last_date ? fmt.date(p.last_date) : "—"}
                    </span>
                    {p.status === "paused" && <span className="pill">paused</span>}
                    {p.openCount > 0 && <span className="pill warn">{p.openCount} open</span>}
                    {p.taskCount > 0 && <span className="pill">{p.taskCount} tasks</span>}
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="home-card">
            <h3 className="section-label">Recently filed</h3>
            {recent.length === 0 ? (
              <p className="dim tiny">Nothing filed yet.</p>
            ) : (
              <ul className="recent-list">
                {recent.map((e) => (
                  <li key={e.id}>
                    <button
                      className="linkbtn"
                      onClick={() =>
                        e.slug
                          ? openEntity(e)
                          : navigate({ type: "day", date: e.date, pane: "note" })
                      }
                    >
                      {e.title || "(untitled)"}
                    </button>
                    <span className="dim tiny">
                      {fmt.date(e.date)} · {e.kind}
                      {e.slug ? ` · ${e.name || e.slug}` : ""}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      )}
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
