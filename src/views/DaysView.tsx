import { useEffect, useState } from "react";
import {
  api,
  errText,
  type DayContent,
  type DayEntry,
  type InboxProcessResult,
} from "../api";
import Markdown from "../Markdown";

type Props = {
  days: DayEntry[];
  vaultPath: string;
  onChanged: () => void;
  onError: (msg: string) => void;
};

export default function DaysView({ days, vaultPath, onChanged, onError }: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<DayContent | null>(null);
  const [pane, setPane] = useState<"note" | "raw">("note");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [pending, setPending] = useState(0);

  useEffect(() => {
    if (!selected && days.length) setSelected(days[0].date);
  }, [days, selected]);

  useEffect(() => {
    if (!selected) return;
    setResult(null);
    setEditing(null);
    api
      .readDay(selected)
      .then((c) => {
        setContent(c);
        setPane(c.note ? "note" : "raw");
      })
      .catch((e) => onError(errText(e)));
    api
      .listInbox()
      .then((items) => setPending(items.filter((i) => i.date === selected).length))
      .catch(() => setPending(0));
  }, [selected, onError]);

  async function process() {
    if (!selected) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processDay(selected);
      setResult(r);
      setContent(await api.readDay(selected));
      setPane("note");
      setPending(0);
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
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  if (!days.length) {
    return (
      <div className="empty">
        <h2>No days yet</h2>
        <p className="dim">
          Capture something into the inbox, then process it. Days appear here once
          anything has been filed.
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
          >
            <div className="row">
              <span className="mono">{d.date}</span>
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
                  }}
                  disabled={!content.note && editing === null}
                >
                  Note
                </button>
                <button
                  className={`tab ${pane === "raw" ? "active" : ""}`}
                  onClick={() => {
                    setPane("raw");
                    setEditing(null);
                  }}
                >
                  Raw
                </button>
              </div>
              <div className="grow" />
              {editing === null ? (
                <button
                  className="btn"
                  onClick={() =>
                    setEditing(pane === "note" ? content.note || "" : content.raw)
                  }
                >
                  Edit {pane}
                </button>
              ) : (
                <>
                  <button className="btn" onClick={() => setEditing(null)}>
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
                onClick={process}
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
                {result.errors.length > 0 && (
                  <> Failed: {result.errors.join(" · ")}</>
                )}
              </div>
            )}

            <div className="content">
              {editing !== null ? (
                <textarea
                  className="rawedit"
                  value={editing}
                  onChange={(e) => setEditing(e.target.value)}
                  spellCheck={false}
                />
              ) : pane === "note" ? (
                <Markdown text={content.note} vaultPath={vaultPath} />
              ) : (
                <pre className="raw">{content.raw || "Nothing archived on this day."}</pre>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
