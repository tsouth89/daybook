import { useCallback, useEffect, useState } from "react";
import { api, errText, type Entry, type ProjectEntry } from "./api";
import EntryEditor, { blankEntry } from "./EntryEditor";
import { ContextMenu, useContextMenu } from "./ContextMenu";
import { useFormat } from "./FormatContext";
import { useNavigate } from "./nav";

type Props = {
  /** Project/area slug, or null for the unowned view. */
  slug: string;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

/**
 * Everything the item layer knows about one project: what is open, what is
 * outstanding, what happened lately. The markdown page above it is the prose;
 * this is the same content as queryable state.
 */
export default function EntryPanel({ slug, onError, onNotice }: Props) {
  const [open, setOpen] = useState<Entry[]>([]);
  const [tasks, setTasks] = useState<Entry[]>([]);
  const [recent, setRecent] = useState<Entry[]>([]);
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [editing, setEditing] = useState<Entry | null>(null);
  const menu = useContextMenu();
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    if (!slug) {
      setOpen([]);
      setTasks([]);
      setRecent([]);
      return;
    }
    try {
      const [loops, open_tasks, latest] = await Promise.all([
        api.queryEntries({ slug, open_only: true, limit: 40 }),
        api.queryEntries({ slug, kind: "task", undone_only: true, limit: 40 }),
        api.queryEntries({ slug, limit: 8 }),
      ]);
      setOpen(loops);
      setTasks(open_tasks);
      setRecent(latest);
      setProjects(await api.listProjects());
    } catch (e) {
      onError(errText(e));
    }
  }, [slug, onError]);

  useEffect(() => {
    load();
  }, [load]);

  async function resolveLoop(entryId: string, line: string) {
    try {
      await api.resolveOpenLoop(entryId, line);
      onNotice?.("Closed");
      await load();
    } catch (e) {
      onError(errText(e));
    }
  }

  async function toggle(t: Entry) {
    try {
      await api.setTaskDone(t.id, !t.done);
      setTasks((prev) => prev.filter((x) => x.id !== t.id));
      onNotice?.(t.done ? "Reopened" : "Done");
    } catch (e) {
      onError(errText(e));
    }
  }

  const loopLines = open.flatMap((e) =>
    e.open.map((line, i) => ({ key: `${e.id}-${i}`, id: e.id, line, date: e.date }))
  );

  if (!slug) return null;

  return (
    <div className="entry-panel">
      {loopLines.length > 0 && (
        <section>
          <h3 className="section-label">Open ({loopLines.length})</h3>
          <ul className="loop-list">
            {loopLines.map((l) => (
              <li key={l.key} className="loop-row">
                <span className="loop-text">{l.line}</span>
                <span className="dim tiny">{fmt.date(l.date)}</span>
                <button
                  className="loop-close"
                  title="Close this loop"
                  onClick={() => void resolveLoop(l.id, l.line)}
                >
                  ✓
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section>
        <h3 className="section-label">
          Tasks{tasks.length > 0 ? ` (${tasks.length})` : ""}
          <button
            className="linkbtn tiny addbtn"
            onClick={() => {
              const draft = blankEntry(new Date().toISOString().slice(0, 10));
              setEditing({ ...draft, slug, kind: "task" });
            }}
          >
            add
          </button>
        </h3>
        <ul className="task-list">
          {tasks.map((t) => (
              <li
                key={t.id}
                className="task-row"
                onContextMenu={(ev) =>
                  menu.open(ev, [{ label: "Edit…", onClick: () => setEditing(t) }])
                }
              >
                <input
                  type="checkbox"
                  checked={t.done}
                  onChange={() => void toggle(t)}
                  aria-label={t.title}
                />
                <span className="task-text" onDoubleClick={() => setEditing(t)}>
                  {t.title}
                </span>
                {t.due && <span className="dim tiny">due {fmt.date(t.due)}</span>}
              </li>
            ))}
        </ul>
        {tasks.length === 0 && <p className="dim tiny">Nothing outstanding here.</p>}
      </section>

      {recent.length > 0 && (
        <section>
          <h3 className="section-label">Recent</h3>
          <ul className="recent-list">
            {recent.map((e) => (
              <li key={e.id}>
                <button
                  className="linkbtn"
                  onClick={() => navigate({ type: "day", date: e.date, pane: "note" })}
                >
                  {e.title || "(untitled)"}
                </button>
                <span className="dim tiny">
                  {fmt.date(e.date)} · {e.kind}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <ContextMenu {...menu.menuProps} />
      <EntryEditor
        entry={editing}
        projects={projects}
        onClose={() => setEditing(null)}
        onSaved={() => void load()}
        onError={onError}
        onNotice={onNotice}
      />
    </div>
  );
}
