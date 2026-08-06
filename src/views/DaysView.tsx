import { useEffect, useRef, useState } from "react";
import {
  api,
  errText,
  type DayContent,
  type DayEntry,
  type InboxProcessResult,
} from "../api";
import ConfirmDialog from "../ConfirmDialog";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";
import NoteEditor from "../NoteEditor";

type Props = {
  days: DayEntry[];
  vaultPath: string;
  focusDate?: string | null;
  focusPane?: "note" | "raw" | null;
  onFocusConsumed?: () => void;
  onChanged: () => void;
  onError: (msg: string) => void;
  onNotice?: (msg: string) => void;
};

export default function DaysView({
  days,
  vaultPath,
  focusDate,
  focusPane,
  onFocusConsumed,
  onChanged,
  onError,
  onNotice,
}: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<DayContent | null>(null);
  const [pane, setPane] = useState<"note" | "raw">("note");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [pending, setPending] = useState(0);
  const [confirmRaw, setConfirmRaw] = useState(false);
  const pendingPane = useRef<"note" | "raw" | null>(null);
  const menu = useContextMenu();
  const fmt = useFormat();

  useEffect(() => {
    if (!selected && days.length) setSelected(days[0].date);
  }, [days, selected]);

  useEffect(() => {
    if (!focusDate) return;
    if (focusPane) pendingPane.current = focusPane;
    setSelected(focusDate);
    onFocusConsumed?.();
  }, [focusDate, focusPane, onFocusConsumed]);

  useEffect(() => {
    if (!selected) return;
    setResult(null);
    setEditing(null);
    setDirty(false);
    api
      .readDay(selected)
      .then((c) => {
        setContent(c);
        const want = pendingPane.current;
        pendingPane.current = null;
        setPane(want ?? (c.note ? "note" : "raw"));
      })
      .catch((e) => onError(errText(e)));
    api
      .listInbox()
      .then((items) => setPending(items.filter((i) => i.date === selected).length))
      .catch(() => setPending(0));
  }, [selected, onError]);

  async function process(date?: string) {
    const d = date ?? selected;
    if (!d) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processDay(d);
      setResult(r);
      if (selected === d) {
        setContent(await api.readDay(d));
        setPane("note");
        setPending(0);
      }
      onChanged();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function saveRaw() {
    if (!selected || editing === null || pane !== "raw") return;
    try {
      await api.writeRaw(selected, editing);
      setContent(await api.readDay(selected));
      setEditing(null);
      setDirty(false);
      onNotice?.("Saved raw (source of truth)");
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  async function saveNote() {
    if (!selected || editing === null || pane !== "note") return;
    try {
      await api.writeNote(selected, editing);
      setContent(await api.readDay(selected));
      setEditing(null);
      setDirty(false);
      onNotice?.("Saved");
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  function startEditNote() {
    if (!content) return;
    setPane("note");
    setEditing(content.note || `# ${selected}\n\n`);
    setDirty(false);
  }

  function startEditRaw() {
    setConfirmRaw(true);
  }

  function confirmStartRaw() {
    if (!content) return;
    setConfirmRaw(false);
    setPane("raw");
    setEditing(content.raw);
    setDirty(false);
  }

  function dayMenu(d: DayEntry) {
    return [
      {
        label: "Open",
        onClick: () => setSelected(d.date),
      },
      {
        label: "Edit note",
        onClick: () => {
          setSelected(d.date);
          api
            .readDay(d.date)
            .then((c) => {
              setContent(c);
              setPane("note");
              setEditing(c.note || `# ${d.date}\n\n`);
              setDirty(false);
            })
            .catch((e) => onError(errText(e)));
        },
      },
      {
        label: "Process day’s inbox",
        disabled: busy,
        onClick: () => {
          setSelected(d.date);
          void process(d.date);
        },
      },
      { kind: "sep" as const },
      {
        label: "Copy date",
        onClick: () => {
          void copyText(fmt.date(d.date));
          onNotice?.("Copied date");
        },
      },
      {
        label: "Reveal day note",
        onClick: () =>
          void api.revealPath(`days/${d.date}.md`).catch((e) => onError(errText(e))),
      },
      {
        label: "Reveal raw archive",
        onClick: () =>
          void api.revealPath(`raw/${d.date}.md`).catch((e) => onError(errText(e))),
      },
    ];
  }

  if (!days.length) {
    return (
      <div className="empty">
        <h2>No days yet</h2>
        <p className="dim">
          Capture something into the inbox, then process it. Days appear here once anything has been
          filed.
        </p>
        <p className="dim" style={{ marginTop: 16 }}>
          <button className="btn primary" onClick={() => api.showCapture()}>
            New entry
          </button>
        </p>
      </div>
    );
  }

  return (
    <div className="split">
      <div className="list">
        {days.map((d) => (
          <button
            key={d.date}
            className={`listitem ${selected === d.date ? "active" : ""}`}
            onClick={() => setSelected(d.date)}
            onContextMenu={(e) => {
              setSelected(d.date);
              menu.open(e, dayMenu(d));
            }}
          >
            <div className="row">
              <span className="mono">{fmt.date(d.date)}</span>
              <span className={`pill ${d.has_note ? "ok" : "pending"}`}>
                {d.has_note ? "note" : "raw"}
              </span>
            </div>
            <div className="preview dim">{d.preview || "—"}</div>
          </button>
        ))}
      </div>

      <div className="detail">
        {content && (
          <>
            <div className="toolbar">
              <div className="tabs">
                <button
                  className={`tab ${pane === "note" ? "active" : ""}`}
                  onClick={() => {
                    setPane("note");
                    setEditing(null);
                    setDirty(false);
                  }}
                >
                  Note
                </button>
                <button
                  className={`tab ${pane === "raw" ? "active" : ""}`}
                  onClick={() => {
                    setPane("raw");
                    setEditing(null);
                    setDirty(false);
                  }}
                >
                  Raw
                </button>
              </div>
              <div className="grow" />
              {dirty && <span className="dim tiny">Unsaved</span>}
              {editing === null ? (
                <button
                  className="btn"
                  onClick={() => (pane === "note" ? startEditNote() : startEditRaw())}
                >
                  Edit {pane}
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
                  <button
                    className="btn primary"
                    onClick={pane === "note" ? saveNote : saveRaw}
                  >
                    Save {pane}
                  </button>
                </>
              )}
              <button
                className="btn primary"
                onClick={() => process()}
                disabled={busy || pending === 0 || editing !== null}
                title={
                  pending === 0
                    ? "No pending inbox items for this day"
                    : `Process ${pending} inbox item(s)`
                }
              >
                {busy
                  ? "Processing…"
                  : pending > 0
                    ? `Process inbox (${pending})`
                    : "Inbox clear"}
              </button>
            </div>

            {result && (
              <div className={`banner ${result.errors.length ? "warn" : "ok"}`}>
                Filed {result.processed.length} capture
                {result.processed.length === 1 ? "" : "s"} for {selected}.
                {result.errors.length > 0 && <> Failed: {result.errors.join(" · ")}</>}
              </div>
            )}

            <div className={`content ${editing !== null && pane === "note" ? "has-editor" : ""}`}>
              {editing !== null && pane === "note" ? (
                <NoteEditor
                  value={editing}
                  onChange={(v) => {
                    setEditing(v);
                    setDirty(v !== (content.note || ""));
                  }}
                  vaultPath={vaultPath}
                  onSave={() => void saveNote()}
                />
              ) : editing !== null && pane === "raw" ? (
                <textarea
                  className="rawedit"
                  value={editing}
                  onChange={(e) => {
                    setEditing(e.target.value);
                    setDirty(e.target.value !== content.raw);
                  }}
                  spellCheck={false}
                />
              ) : pane === "note" ? (
                content.note ? (
                  <Markdown
                    text={content.note}
                    vaultPath={vaultPath}
                    onEdit={startEditNote}
                    extraMenu={
                      selected
                        ? [
                            {
                              label: "Reveal day note",
                              onClick: () =>
                                void api
                                  .revealPath(`days/${selected}.md`)
                                  .catch((e) => onError(errText(e))),
                            },
                            {
                              label: "Copy date",
                              onClick: () => {
                                void copyText(fmt.date(selected));
                                onNotice?.("Copied date");
                              },
                            },
                          ]
                        : undefined
                    }
                  />
                ) : (
                  <div className="empty" style={{ padding: 0 }}>
                    <h2>No day note yet</h2>
                    <p className="dim">
                      Process inbox items to generate one, or write a note yourself.
                    </p>
                    <p className="dim" style={{ marginTop: 16 }}>
                      <button className="btn primary" onClick={startEditNote}>
                        Write note
                      </button>
                    </p>
                  </div>
                )
              ) : (
                <pre
                  className="raw"
                  onContextMenu={(e) => {
                    menu.open(e, [
                      {
                        label: "Copy all",
                        onClick: () => {
                          void copyText(content.raw);
                          onNotice?.("Copied raw");
                        },
                      },
                      {
                        label: "Edit raw…",
                        onClick: startEditRaw,
                      },
                      {
                        label: "Reveal raw archive",
                        onClick: () =>
                          selected &&
                          void api
                            .revealPath(`raw/${selected}.md`)
                            .catch((err) => onError(errText(err))),
                      },
                    ]);
                  }}
                >
                  {content.raw || "Nothing archived on this day."}
                </pre>
              )}
            </div>
          </>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
      <ConfirmDialog
        open={confirmRaw}
        title="Edit raw archive?"
        body="Raw is the append-only source of truth. Overwriting it can destroy captures that day notes are rebuilt from. Prefer editing the Note pane unless you know what you’re doing."
        confirmLabel="Edit raw anyway"
        danger
        onCancel={() => setConfirmRaw(false)}
        onConfirm={confirmStartRaw}
      />
    </div>
  );
}
