import { useCallback, useEffect, useState } from "react";
import { api, errText, type HistoryItem } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
  onOpenDay?: (date: string) => void;
};

export default function HistoryView({
  vaultPath,
  onError,
  onNotice,
  onOpenDay,
}: Props) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [body, setBody] = useState("");
  const menu = useContextMenu();
  const fmt = useFormat();

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

  function historyMenu(item: HistoryItem) {
    return [
      {
        label: "Open",
        onClick: () => setSelected(keyOf(item)),
      },
      {
        label: "Open day",
        disabled: !onOpenDay,
        onClick: () => onOpenDay?.(item.date),
      },
      { kind: "sep" as const },
      {
        label: "Copy text",
        onClick: () => {
          void (async () => {
            try {
              const t =
                selected === keyOf(item)
                  ? body
                  : await api.readHistoryItem(item.date, item.id);
              await copyText(t);
              onNotice?.("Copied capture");
            } catch (e) {
              onError(errText(e));
            }
          })();
        },
      },
      {
        label: "Copy date",
        onClick: () => {
          void copyText(fmt.date(item.date));
          onNotice?.("Copied date");
        },
      },
      {
        label: "Reveal raw archive",
        onClick: () =>
          void api.revealPath(`raw/${item.date}.md`).catch((e) => onError(errText(e))),
      },
    ];
  }

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
              onContextMenu={(e) => {
                setSelected(k);
                menu.open(e, historyMenu(item));
              }}
            >
              <div className="row">
                <span className="mono">
                  {fmt.dateTime(item.date, item.time || "")}
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
                {fmt.dateTime(current.date, current.time || "")}
                {current.id ? ` · ${current.id}` : ""}
              </div>
              <div className="grow" />
              {onOpenDay && (
                <button className="btn" onClick={() => onOpenDay(current.date)}>
                  Open day
                </button>
              )}
              <span className="dim tiny">{current.chars} chars</span>
            </div>
            <div className="content">
              <Markdown
                text={body}
                vaultPath={vaultPath}
                extraMenu={historyMenu(current)}
              />
            </div>
          </>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}

function keyOf(item: HistoryItem): string {
  return `${item.date}|${item.time}|${item.id}|${item.chars}`;
}
