import { useCallback, useEffect, useState } from "react";
import { api, errText, type HistoryItem } from "../api";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
};

export default function HistoryView({ vaultPath, onError }: Props) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [body, setBody] = useState("");

  const refresh = useCallback(async () => {
    try {
      const list = await api.listHistory();
      setItems(list);
      setSelected((cur) => {
        if (cur && list.some((i) => keyOf(i) === cur)) return cur;
        return list[0] ? keyOf(list[0]) : null;
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

  useEffect(() => {
    if (!selected) {
      setBody("");
      return;
    }
    const item = items.find((i) => keyOf(i) === selected);
    if (!item) return;
    api
      .readHistoryItem(item.date, item.id)
      .then(setBody)
      .catch((e) => onError(errText(e)));
  }, [selected, items, onError]);

  if (!items.length) {
    return (
      <div className="empty">
        <h2>No capture history yet</h2>
        <p className="dim">
          After you process the inbox, verbatim dumps land in <span className="mono">raw/</span> and
          show up here — what you said, when, and whether it was filed into a day note.
        </p>
      </div>
    );
  }

  const current = items.find((i) => keyOf(i) === selected);

  return (
    <div className="split">
      <div className="list">
        {items.map((item) => {
          const k = keyOf(item);
          return (
            <button
              key={k}
              className={`listitem ${selected === k ? "active" : ""}`}
              onClick={() => setSelected(k)}
            >
              <div className="row">
                <span className="mono">
                  {item.date} {item.time || "—"}
                </span>
                <span className={`pill ${item.has_day_note ? "ok" : "pending"}`}>
                  {item.has_day_note ? "filed" : "raw"}
                </span>
              </div>
              <div className="preview dim">{item.preview || "—"}</div>
            </button>
          );
        })}
      </div>
      <div className="detail">
        {current && (
          <>
            <div className="toolbar">
              <div className="mono dim">
                {current.date} {current.time}
                {current.id ? ` · ${current.id}` : ""}
              </div>
              <div className="grow" />
              <span className="dim tiny">{current.chars} chars</span>
            </div>
            <div className="content">
              <Markdown text={body} vaultPath={vaultPath} />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function keyOf(item: HistoryItem): string {
  return `${item.date}|${item.time}|${item.id}|${item.chars}`;
}
