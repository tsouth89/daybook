import { useCallback, useEffect, useState } from "react";
import { api, errText, type InboxItem, type InboxProcessResult } from "../api";
import ConfirmDialog from "../ConfirmDialog";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";
import ProcessResult from "../ProcessResult";
import { useViewHandlers } from "../viewhost";
import NoteEditor from "../NoteEditor";

type Props = {
  vaultPath: string;
  onChanged: () => void;
  onError: (msg: string) => void;
  onNotice?: (msg: string) => void;
};

export default function InboxView({
  vaultPath,
  onChanged,
  onError,
  onNotice,
}: Props) {
  const [items, setItems] = useState<InboxItem[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const menu = useContextMenu();
  const fmt = useFormat();

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
    const onFocus = () => {
      if (editing === null) refresh();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh, editing]);

  useEffect(() => {
    setEditing(null);
    setDirty(false);
  }, [selected]);

  const current = items.find((i) => i.id === selected) ?? null;

  useViewHandlers({
    isDirty: () => editing !== null && dirty,
    process: () => void processAll(),
  });

  async function processAll() {
    if (dirty && editing !== null && selected) {
      await saveEdit();
    }
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

  async function processOne(id: string) {
    if (dirty && editing !== null && selected === id) {
      await saveEdit();
    }
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processInboxItem(id);
      setResult(r);
      await refresh();
      onChanged();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function saveEdit() {
    if (!selected || editing === null) return;
    try {
      await api.updateInboxItem(selected, editing);
      setItems((prev) =>
        prev.map((i) =>
          i.id === selected
            ? { ...i, text: editing, chars: editing.length }
            : i
        )
      );
      setDirty(false);
      setEditing(null);
      onNotice?.("Saved capture");
    } catch (e) {
      onError(errText(e));
    }
  }

  async function discard(id: string) {
    try {
      await api.deleteInboxItem(id);
      setConfirmDiscard(null);
      await refresh();
      onChanged();
    } catch (e) {
      onError(errText(e));
    }
  }

  function itemMenu(item: InboxItem) {
    return [
      {
        label: "Edit",
        onClick: () => {
          setSelected(item.id);
          setEditing(item.text);
          setDirty(false);
        },
      },
      {
        label: "Process this item",
        disabled: busy,
        onClick: () => void processOne(item.id),
      },
      {
        label: "Copy text",
        onClick: () => {
          void copyText(item.text);
          onNotice?.("Copied capture text");
        },
      },
      { kind: "sep" as const },
      {
        label: "Reveal in vault",
        onClick: () =>
          void api.revealPath(`inbox/${item.id}.md`).catch((e) => onError(errText(e))),
      },
      {
        label: "Discard",
        danger: true,
        onClick: () => setConfirmDiscard(item.id),
      },
    ];
  }

  if (!items.length && !result) {
    return (
      <div className="empty">
        <h2>Inbox empty</h2>
        <p className="dim">
          Captures land here first. Hit{" "}
          <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd>, dump a thought, then process when you
          want them filed.
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
      <div
        className="list"
        onContextMenu={(e) => {
          if ((e.target as HTMLElement).closest(".listitem")) return;
          menu.open(e, [
            {
              label: "Process inbox",
              disabled: busy || items.length === 0,
              onClick: () => void processAll(),
            },
            {
              label: "New entry",
              onClick: () => void api.showCapture(),
            },
          ]);
        }}
      >
        <div className="list-head">
          <span className="dim tiny">{items.length} pending</span>
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
            onContextMenu={(e) => {
              setSelected(item.id);
              menu.open(e, itemMenu(item));
            }}
          >
            <div className="row">
              <span className="mono">{fmt.dateTime(item.date, item.time)}</span>
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
        {result && <ProcessResult result={result} />}

        {current ? (
          <>
            <div className="toolbar">
              <div className="mono dim">
                {fmt.dateTime(current.date, current.time)} · {current.id}
                {dirty ? " · unsaved" : ""}
              </div>
              <div className="grow" />
              {editing === null ? (
                <>
                  <button
                    className="btn"
                    onClick={() => {
                      setEditing(current.text);
                      setDirty(false);
                    }}
                  >
                    Edit
                  </button>
                  <button
                    className="btn primary"
                    onClick={() => processOne(current.id)}
                    disabled={busy}
                  >
                    {busy ? "Processing…" : "Process"}
                  </button>
                  <button className="btn danger" onClick={() => setConfirmDiscard(current.id)}>
                    Discard
                  </button>
                </>
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
                  <button className="btn primary" onClick={() => void saveEdit()}>
                    Save
                  </button>
                </>
              )}
            </div>
            <div className={`content ${editing !== null ? "has-editor" : ""}`}>
              {editing !== null ? (
                <NoteEditor
                  value={editing}
                  onChange={(v) => {
                    setEditing(v);
                    setDirty(v !== current.text);
                  }}
                  vaultPath={vaultPath}
                  onSave={() => void saveEdit()}
                  initialMode="source"
                />
              ) : (
                <Markdown
                  text={current.text}
                  vaultPath={vaultPath}
                  onEdit={() => {
                    setEditing(current.text);
                    setDirty(false);
                  }}
                  extraMenu={itemMenu(current)}
                />
              )}
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

      <ContextMenu {...menu.menuProps} />
      <ConfirmDialog
        open={!!confirmDiscard}
        title="Discard capture?"
        body="This removes the inbox item permanently. It will not be filed into raw or day notes."
        confirmLabel="Discard"
        danger
        onCancel={() => setConfirmDiscard(null)}
        onConfirm={() => confirmDiscard && void discard(confirmDiscard)}
      />
    </div>
  );
}
