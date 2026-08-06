import { useCallback, useEffect, useState } from "react";
import { api, errText, type InboxItem, type InboxProcessResult } from "../api";
import Backlinks from "../Backlinks";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";
import NoteEditor from "../NoteEditor";
import ProcessResult from "../ProcessResult";
import { useNavigate } from "../nav";
import { useViewHandlers } from "../viewhost";

type Props = {
  /** ISO date for today, from the backend clock. */
  date: string;
  vaultPath: string;
  onChanged: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

/** Landing view: today's day note plus whatever is still waiting in the inbox. */
export default function TodayView({ date, vaultPath, onChanged, onError, onNotice }: Props) {
  const [note, setNote] = useState("");
  const [pending, setPending] = useState<InboxItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const menu = useContextMenu();
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    try {
      const c = await api.readDay(date);
      setNote(c.note);
    } catch (e) {
      onError(errText(e));
    }
    try {
      const items = await api.listInbox();
      setPending(items.filter((i) => i.date === date));
    } catch {
      setPending([]);
    }
  }, [date, onError]);

  useEffect(() => {
    setEditing(null);
    setDirty(false);
    setResult(null);
    load();
  }, [load]);

  useEffect(() => {
    const onFocus = () => {
      if (editing === null) load();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load, editing]);

  const process = useCallback(async () => {
    if (busy || !pending.length) return;
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
  }, [busy, pending.length, date, load, onChanged, onError]);

  useViewHandlers({
    isDirty: () => editing !== null && dirty,
    process: () => void process(),
  });

  async function startEdit() {
    try {
      // Creates the scaffold on demand so there is always something to type into.
      await api.ensureDay(date);
      const c = await api.readDay(date);
      setNote(c.note);
      setEditing(c.note);
      setDirty(false);
    } catch (e) {
      onError(errText(e));
    }
  }

  async function save() {
    if (editing === null) return;
    try {
      await api.writeNote(date, editing);
      setNote(editing);
      setEditing(null);
      setDirty(false);
      onNotice?.("Saved");
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  const pageMenu = [
    {
      label: "New entry",
      onClick: () => void api.showCapture(),
    },
    {
      label: `Process pending (${pending.length})`,
      disabled: busy || pending.length === 0,
      onClick: () => void process(),
    },
    { kind: "sep" as const },
    {
      label: "Open in Days",
      onClick: () => navigate({ type: "day", date, pane: "note" }),
    },
    {
      label: "Copy date",
      onClick: () => {
        void copyText(fmt.date(date));
        onNotice?.("Copied date");
      },
    },
    {
      label: "Reveal day note",
      onClick: () => void api.revealPath(`days/${date}.md`).catch((e) => onError(errText(e))),
    },
  ];

  return (
    <div
      className="detail"
      style={{ flex: 1 }}
      onContextMenu={(e) => {
        if ((e.target as HTMLElement).closest(".md, .cm-editor, .ctxmenu, .note-editor")) return;
        menu.open(e, pageMenu);
      }}
    >
      <div className="toolbar">
        <div className="mono dim">
          {fmt.date(date)}
          {dirty ? " · unsaved" : ""}
        </div>
        <div className="grow" />
        <button className="btn" onClick={() => api.showCapture()}>
          New entry
        </button>
        {editing === null ? (
          <button className="btn" onClick={startEdit}>
            Edit note
          </button>
        ) : (
          <>
            <button
              className="btn"
              onClick={() => {
                setEditing(null);
                setDirty(false);
              }}
            >
              Cancel
            </button>
            <button className="btn primary" onClick={save}>
              Save
            </button>
          </>
        )}
        <button
          className="btn primary"
          onClick={() => void process()}
          disabled={busy || pending.length === 0 || editing !== null}
          title={
            pending.length === 0
              ? "Nothing pending for today"
              : `Process ${pending.length} inbox item(s) · Ctrl+Shift+P`
          }
        >
          {busy
            ? "Processing…"
            : pending.length > 0
              ? `Process pending (${pending.length})`
              : "Inbox clear"}
        </button>
      </div>

      {result && <ProcessResult result={result} label="Filed" />}

      <div className={`content ${editing !== null ? "has-editor" : ""}`}>
        {editing !== null ? (
          <NoteEditor
            value={editing}
            onChange={(v) => {
              setEditing(v);
              setDirty(v !== note);
            }}
            vaultPath={vaultPath}
            onSave={() => void save()}
          />
        ) : note ? (
          <>
            <Markdown text={note} vaultPath={vaultPath} onEdit={startEdit} extraMenu={pageMenu} />
            <Backlinks target={`days/${date}`} onError={onError} />
          </>
        ) : (
          <div className="empty" style={{ padding: 0 }}>
            <h2>Nothing filed today yet</h2>
            <p className="dim">
              {pending.length > 0
                ? `${pending.length} capture${pending.length === 1 ? "" : "s"} waiting in the inbox.`
                : "Capture a thought, then process it into today’s note."}
            </p>
            <p className="dim row gap" style={{ marginTop: 16 }}>
              <button className="btn primary" onClick={() => api.showCapture()}>
                New entry
              </button>
              {pending.length > 0 && (
                <button className="btn" onClick={() => void process()} disabled={busy}>
                  {busy ? "Processing…" : `Process pending (${pending.length})`}
                </button>
              )}
              <button className="btn" onClick={startEdit}>
                Write note
              </button>
            </p>
          </div>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
