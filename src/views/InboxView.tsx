import { useCallback, useEffect, useState } from "react";
import {
  api,
  errText,
  type InboxItem,
  type InboxProcessResult,
} from "../api";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onChanged: () => void;
  onError: (msg: string) => void;
};

export default function InboxView({ vaultPath, onChanged, onError }: Props) {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await api.listInbox();
      setItems(list);
      setSelected((cur) => {
        if (cur && list.some((i) => i.id === cur)) return cur;
        return list[0]?.id ?? null;
      });
    } catch (e) {
      onError(errText(e));
    }
  }, [onError]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  const current = items.find((i) => i.id === selected) ?? null;

  async function processAll() {
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processInbox();
      setResult(r);
      await refresh();
      onChanged();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function discard(id: string) {
    try {
      await api.deleteInboxItem(id);
      await refresh();
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  if (!items.length && !result) {
    return (
      <div className="empty">
        <h2>Inbox empty</h2>
        <p className="dim">
          Captures land here first. Hit{" "}
          <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd>, dump a thought, then
          process when you want them filed.
        </p>
      </div>
    );
  }

  return (
    <div className="split">
      <div className="list">
        <div className="list-head">
          <span className="dim tiny">
            {items.length} pending
          </span>
          <button
            className="btn primary"
            onClick={processAll}
            disabled={busy || items.length === 0}
          >
            {busy ? "Processing…" : "Process inbox"}
          </button>
        </div>
        {items.map((item) => (
          <button
            key={item.id}
            className={`listitem ${selected === item.id ? "active" : ""}`}
            onClick={() => setSelected(item.id)}
          >
            <div className="row">
              <span className="mono">
                {item.date} {item.time}
              </span>
              <span className="pill pending">{item.chars}c</span>
            </div>
            <div className="preview dim">
              {item.text.slice(0, 120) || "—"}
              {item.text.length > 120 ? "…" : ""}
            </div>
          </button>
        ))}
      </div>

      <div className="detail">
        {result && (
          <div className={`banner ${result.errors.length ? "warn" : "ok"}`}>
            Processed {result.processed.length} item
            {result.processed.length === 1 ? "" : "s"}
            {result.processed.length > 0 && (
              <>
                :{" "}
                {result.processed
                  .map((p) => `${p.entry_count} entr${p.entry_count === 1 ? "y" : "ies"}`)
                  .join(", ")}
                .
              </>
            )}
            {result.processed.some((p) => p.new_entities.length > 0) && (
              <>
                {" "}
                New:{" "}
                <strong>
                  {[
                    ...new Set(
                      result.processed.flatMap((p) => p.new_entities)
                    ),
                  ].join(", ")}
                </strong>
                .
              </>
            )}
            {result.errors.length > 0 && (
              <>
                {" "}
                {result.errors.length} failed (still in inbox):{" "}
                {result.errors.join(" · ")}
              </>
            )}
          </div>
        )}

        {current ? (
          <>
            <div className="toolbar">
              <div className="mono dim">
                {current.date} {current.time} · {current.id}
              </div>
              <div className="grow" />
              <button className="btn" onClick={() => discard(current.id)}>
                Discard
              </button>
            </div>
            <div className="content">
              <Markdown text={current.text} vaultPath={vaultPath} />
            </div>
          </>
        ) : (
          !busy &&
          items.length === 0 && (
            <div className="empty">
              <h2>All clear</h2>
              <p className="dim">Everything in the inbox has been filed.</p>
            </div>
          )
        )}
      </div>
    </div>
  );
}
