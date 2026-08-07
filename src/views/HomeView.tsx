import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  errText,
  type Entry,
  type InboxProcessResult,
  type ProjectSummary,
} from "../api";
import { ContextMenu, useContextMenu } from "../ContextMenu";
import EntryEditor, { blankEntry } from "../EntryEditor";
import { useFormat } from "../FormatContext";
import ProcessResult from "../ProcessResult";
import { useNavigate } from "../nav";
import { useViewHandlers } from "../viewhost";

type Props = {
  /** ISO date for today, from the backend clock. */
  date: string;
  /** Bumped when background routing lands, so this re-queries. */
  refreshTick?: number;
  onChanged: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

/**
 * Your projects, at a glance.
 *
 * The previous version was four equal cards — open loops, tasks, projects,
 * recently filed — which is Daybook's data model rather than anything you
 * care about. Loops and tasks are both "stuff I owe", so they belong to the
 * project they're on; "recently filed" is a log, which is Today's job.
 */
export default function HomeView({ date, refreshTick, onChanged, onError, onNotice }: Props) {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [loose, setLoose] = useState<Entry[]>([]);
  const [pending, setPending] = useState(0);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [editing, setEditing] = useState<Entry | null>(null);
  const [describing, setDescribing] = useState<string | null>(null);
  const [draftAbout, setDraftAbout] = useState("");
  const menu = useContextMenu();
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    try {
      const [summaries, unowned, inbox] = await Promise.all([
        api.projectSummaries(),
        api.queryEntries({ kind: "task", undone_only: true, limit: 100 }),
        api.listInbox(),
      ]);
      setProjects(summaries);
      setLoose(unowned.filter((t) => !t.slug));
      setPending(inbox.filter((i) => i.date === date).length);
      return summaries.length + unowned.length;
    } catch (e) {
      onError(errText(e));
      return 0;
    }
  }, [date, onError]);

  useEffect(() => {
    load();
  }, [load, refreshTick]);

  // First run against an existing vault: recovering costs nothing, so do it
  // rather than showing an empty page and asking.
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

  const active = useMemo(
    () =>
      projects
        .filter((p) => p.status !== "done")
        .sort(
          (a, b) =>
            (a.status === "paused" ? 1 : 0) - (b.status === "paused" ? 1 : 0) ||
            b.overdue_tasks - a.overdue_tasks ||
            (b.last_date || "").localeCompare(a.last_date || "")
        ),
    [projects]
  );

  const overdue = useMemo(
    () => projects.reduce((n, p) => n + p.overdue_tasks, 0),
    [projects]
  );

  function open(p: ProjectSummary) {
    navigate({ type: "entity", kind: p.kind === "area" ? "area" : "project", slug: p.slug });
  }

  async function act(fn: () => Promise<unknown>, msg?: string) {
    try {
      await fn();
      if (msg) onNotice?.(msg);
      await load();
    } catch (e) {
      onError(errText(e));
    }
  }

  async function saveAbout(p: ProjectSummary) {
    const text = draftAbout.trim();
    setDescribing(null);
    if (!text || text === p.about) return;
    await act(() => api.setEntityAbout(p.kind, p.slug, text), "Saved");
  }

  if (scanning) {
    return <div className="content home"><p className="dim">Scanning the vault…</p></div>;
  }

  return (
    <div
      className="content home"
      onContextMenu={(e) => {
        if ((e.target as HTMLElement).closest("button, input, textarea, .ctxmenu")) return;
        menu.open(e, [
          { label: "Capture…", onClick: () => void api.showCapture() },
          { label: "Add task by hand…", onClick: () => setEditing(blankEntry(date)) },
          {
            label: `Process waiting (${pending})`,
            disabled: busy || pending === 0,
            onClick: () => void process(),
          },
          { kind: "sep" },
          {
            label: "Rescan vault for entries",
            onClick: () =>
              void act(
                () => api.rebuildEntryIndex(),
                "Rescanned"
              ),
          },
        ]);
      }}
    >
      {/* A strip, not a card: only the things that have earned an interruption. */}
      <div className="home-strip">
        <div className="home-strip-main">
          <span className="home-date">{fmt.date(date)}</span>
          {pending > 0 ? (
            <span className="strip-flag warn">
              {pending} capture{pending === 1 ? "" : "s"} waiting
            </span>
          ) : (
            <span className="dim tiny">Nothing waiting</span>
          )}
          {overdue > 0 && <span className="strip-flag bad">{overdue} overdue</span>}
        </div>
        <button className="btn" onClick={() => api.showCapture()}>
          Capture
        </button>
        <button className="btn" onClick={() => setEditing(blankEntry(date))}>
          Add task
        </button>
        {pending > 0 && (
          <button className="btn primary" onClick={() => void process()} disabled={busy}>
            {busy ? "Filing…" : "File them"}
          </button>
        )}
      </div>

      {result && <ProcessResult result={result} label="Filed" />}

      {active.length === 0 && loose.length === 0 ? (
        <div className="empty" style={{ padding: "24px 0" }}>
          <h2>No projects yet</h2>
          <p className="dim">
            Capture something and Daybook will file it. Projects appear here as they get
            created, with what each one is and what it's waiting on.
          </p>
          <p style={{ marginTop: 16 }}>
            <button className="btn primary" onClick={() => api.showCapture()}>
              Capture
            </button>
          </p>
        </div>
      ) : (
        <div className="proj-grid">
          {active.map((p) => {
            const doneCount = p.objectives.filter((o) => o.done).length;
            return (
              <section
                key={`${p.kind}:${p.slug}`}
                className="proj-card"
                onContextMenu={(e) => {
                  e.stopPropagation();
                  menu.open(e, [
                    { label: `Open ${p.name}`, onClick: () => open(p) },
                    {
                      label: "Describe…",
                      onClick: () => {
                        setDraftAbout(p.about);
                        setDescribing(p.slug);
                      },
                    },
                    {
                      label: "Add a task here…",
                      onClick: () =>
                        setEditing({ ...blankEntry(date), slug: p.slug, kind: "task" }),
                    },
                  ]);
                }}
              >
                <div className="proj-head">
                  <button className="proj-name" onClick={() => open(p)}>
                    {p.name}
                  </button>
                  {p.status !== "active" && <span className="pill">{p.status}</span>}
                  <span className="grow" />
                  {p.last_date && (
                    <span className="dim tiny">{fmt.date(p.last_date)}</span>
                  )}
                </div>

                {describing === p.slug ? (
                  <textarea
                    className="proj-about-edit"
                    autoFocus
                    value={draftAbout}
                    onChange={(e) => setDraftAbout(e.target.value)}
                    onBlur={() => void saveAbout(p)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.shiftKey) {
                        e.preventDefault();
                        void saveAbout(p);
                      }
                      if (e.key === "Escape") setDescribing(null);
                    }}
                    placeholder="What is this project?"
                  />
                ) : (
                  <p
                    className={`proj-about ${p.about ? "" : "dim"}`}
                    onClick={() => {
                      setDraftAbout(p.about);
                      setDescribing(p.slug);
                    }}
                    title="Click to edit"
                  >
                    {p.about || "Say what this is…"}
                  </p>
                )}

                {p.objectives.length > 0 && (
                  <div className="proj-block">
                    <div className="proj-block-head">
                      <span className="section-label">Objectives</span>
                      <span className="dim tiny">
                        {doneCount} of {p.objectives.length}
                      </span>
                    </div>
                    <ul className="obj-list">
                      {p.objectives.map((o, i) => (
                        <li key={i}>
                          <input
                            type="checkbox"
                            checked={o.done}
                            onChange={() =>
                              void act(() =>
                                api.setObjectiveDone(p.kind, p.slug, i, !o.done)
                              )
                            }
                            aria-label={o.text}
                          />
                          <span className={o.done ? "struck" : ""}>{o.text}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                {p.now.length > 0 && (
                  <div className="proj-block">
                    <span className="section-label">Now</span>
                    <ul className="loop-list">
                      {p.now.map((line, i) => (
                        <li key={i}>{line}</li>
                      ))}
                    </ul>
                  </div>
                )}

                <div className="proj-foot">
                  {p.open_tasks > 0 ? (
                    <button className="linkbtn tiny" onClick={() => open(p)}>
                      {p.open_tasks} open task{p.open_tasks === 1 ? "" : "s"}
                    </button>
                  ) : (
                    <span className="dim tiny">No open tasks</span>
                  )}
                  {p.overdue_tasks > 0 && (
                    <span className="pill bad">{p.overdue_tasks} overdue</span>
                  )}
                </div>
              </section>
            );
          })}
        </div>
      )}

      {loose.length > 0 && (
        <section className="loose">
          <h3 className="section-label">Not on a project ({loose.length})</h3>
          <ul className="task-list">
            {loose.map((t) => (
              <li
                key={t.id}
                className="task-row"
                onContextMenu={(e) => {
                  e.stopPropagation();
                  menu.open(e, [{ label: "Edit…", onClick: () => setEditing(t) }]);
                }}
              >
                <input
                  type="checkbox"
                  checked={t.done}
                  onChange={() => void act(() => api.setTaskDone(t.id, !t.done))}
                  aria-label={t.title}
                />
                <span className="task-text" onDoubleClick={() => setEditing(t)}>
                  {t.title}
                </span>
                {t.due && (
                  <span className={`dim tiny ${t.due < date ? "bad" : ""}`}>
                    {fmt.date(t.due)}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      <ContextMenu {...menu.menuProps} />
      <EntryEditor
        entry={editing}
        projects={projects.map((p) => ({
          slug: p.slug,
          name: p.name,
          kind: p.kind,
          scope: p.scope,
          status: p.status,
          parent: p.parent,
          last_date: p.last_date,
          day_count: 0,
        }))}
        onClose={() => setEditing(null)}
        onSaved={() => void load()}
        onError={onError}
        onNotice={onNotice}
      />
    </div>
  );
}
