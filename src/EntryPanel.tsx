import { useCallback, useEffect, useState } from "react";
import { api, errText, type Entry } from "./api";
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
    } catch (e) {
      onError(errText(e));
    }
  }, [slug, onError]);

  useEffect(() => {
    load();
  }, [load]);

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
    e.open.map((line, i) => ({ key: `${e.id}-${i}`, line, date: e.date }))
  );

  if (!slug || (loopLines.length === 0 && tasks.length === 0 && recent.length === 0)) {
    return null;
  }

  return (
    <div className="entry-panel">
      {loopLines.length > 0 && (
        <section>
          <h3 className="section-label">Open ({loopLines.length})</h3>
          <ul className="loop-list">
            {loopLines.map((l) => (
              <li key={l.key}>
                {l.line}
                <span className="dim tiny"> · {fmt.date(l.date)}</span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {tasks.length > 0 && (
        <section>
          <h3 className="section-label">Tasks ({tasks.length})</h3>
          <ul className="task-list">
            {tasks.map((t) => (
              <li key={t.id} className="task-row">
                <input
                  type="checkbox"
                  checked={t.done}
                  onChange={() => void toggle(t)}
                  aria-label={t.title}
                />
                <span className="task-text">{t.title}</span>
                {t.due && <span className="dim tiny">due {fmt.date(t.due)}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}

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
    </div>
  );
}
