import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";

type Props = {
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

type TaskLine = {
  line: number;
  done: boolean;
  text: string;
};

function parseTasks(md: string): TaskLine[] {
  const out: TaskLine[] = [];
  md.split("\n").forEach((raw, i) => {
    const line = i + 1;
    const open = raw.match(/^- \[ \] (.+)/);
    const done = raw.match(/^- \[[xX]\] (.+)/);
    if (open) out.push({ line, done: false, text: open[1] });
    else if (done) out.push({ line, done: true, text: done[1] });
  });
  return out;
}

export default function TasksView({ onError, onNotice }: Props) {
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const menu = useContextMenu();
  const tasks = parseTasks(body);
  const open = tasks.filter((t) => !t.done);
  const done = tasks.filter((t) => t.done);

  const refresh = useCallback(async () => {
    try {
      setBody(await api.readTasks());
    } catch (e) {
      onError(errText(e));
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const onFocus = () => {
      if (editing === null) refresh();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh, editing]);

  async function toggle(line: number) {
    try {
      setBody(await api.toggleTaskLine(line));
    } catch (e) {
      onError(errText(e));
    }
  }

  async function save() {
    if (editing === null) return;
    try {
      await api.writeTasks(editing);
      setBody(editing);
      setEditing(null);
    } catch (e) {
      onError(errText(e));
    }
  }

  function taskMenu(t: TaskLine) {
    return [
      {
        label: t.done ? "Mark open" : "Mark done",
        onClick: () => void toggle(t.line),
      },
      {
        label: "Copy text",
        onClick: () => {
          void copyText(t.text);
          onNotice?.("Copied task");
        },
      },
      { kind: "sep" as const },
      {
        label: "Edit markdown",
        onClick: () => setEditing(body || "# Tasks\n\n"),
      },
      {
        label: "Reveal in vault",
        onClick: () => void api.revealPath("tasks.md").catch((e) => onError(errText(e))),
      },
    ];
  }

  return (
    <div
      className="detail"
      style={{ flex: 1 }}
      onContextMenu={(e) => {
        if ((e.target as HTMLElement).closest("li, textarea, .ctxmenu")) return;
        menu.open(e, [
          {
            label: "Edit markdown",
            onClick: () => setEditing(body || "# Tasks\n\n"),
          },
          {
            label: "Copy all",
            onClick: () => {
              void copyText(body);
              onNotice?.("Copied tasks");
            },
          },
          {
            label: "Reveal in vault",
            onClick: () => void api.revealPath("tasks.md").catch((err) => onError(errText(err))),
          },
        ]);
      }}
    >
      <div className="toolbar">
        <div className="dim tiny">Tasks</div>
        <div className="grow" />
        {editing === null ? (
          <button className="btn" onClick={() => setEditing(body || "# Tasks\n\n")}>
            Edit markdown
          </button>
        ) : (
          <>
            <button className="btn" onClick={() => setEditing(null)}>
              Cancel
            </button>
            <button className="btn primary" onClick={save}>
              Save
            </button>
          </>
        )}
      </div>
      <div className="content tasks-view">
        {editing !== null ? (
          <textarea
            className="rawedit"
            value={editing}
            onChange={(e) => setEditing(e.target.value)}
            spellCheck={false}
          />
        ) : !tasks.length ? (
          <div className="empty" style={{ padding: 0 }}>
            <h2>No tasks yet</h2>
            <p className="dim">
              Tasks appear from triage, or Edit markdown and add{" "}
              <span className="mono">- [ ] …</span> checkboxes yourself.
            </p>
            <p className="dim" style={{ marginTop: 16 }}>
              <button className="btn" onClick={() => setEditing("# Tasks\n\n- [ ] ")}>
                Add a task
              </button>
            </p>
          </div>
        ) : (
          <>
            {open.length > 0 && (
              <section>
                <h3 className="section-label">Open ({open.length})</h3>
                <ul className="tasklist">
                  {open.map((t) => (
                    <li
                      key={t.line}
                      onContextMenu={(e) => menu.open(e, taskMenu(t))}
                    >
                      <label>
                        <input type="checkbox" checked={false} onChange={() => toggle(t.line)} />
                        <span>{t.text}</span>
                      </label>
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {done.length > 0 && (
              <section>
                <h3 className="section-label dim">Done ({done.length})</h3>
                <ul className="tasklist done">
                  {done.map((t) => (
                    <li
                      key={t.line}
                      onContextMenu={(e) => menu.open(e, taskMenu(t))}
                    >
                      <label>
                        <input type="checkbox" checked onChange={() => toggle(t.line)} />
                        <span>{t.text}</span>
                      </label>
                    </li>
                  ))}
                </ul>
              </section>
            )}
          </>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
