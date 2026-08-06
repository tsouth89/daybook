import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import Markdown from "../Markdown";
import NoteEditor from "../NoteEditor";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

export default function PersonalView({ vaultPath, onError, onNotice }: Props) {
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState(false);
  const menu = useContextMenu();

  const refresh = useCallback(async () => {
    try {
      setBody(await api.readPersonal());
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

  async function save() {
    if (editing === null) return;
    try {
      await api.writePersonal(editing);
      setBody(editing);
      setEditing(null);
      setDirty(false);
      onNotice?.("Saved");
    } catch (e) {
      onError(errText(e));
    }
  }

  function cancel() {
    setEditing(null);
    setDirty(false);
  }

  async function refreshOverview() {
    setBusy(true);
    try {
      const next = await api.refreshPersonalOverview();
      setBody(next);
      if (editing !== null) {
        setEditing(next);
        setDirty(true);
      }
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  const hasContent = body.split("\n").some((l) => {
    const t = l.trim();
    return t.startsWith("## ") || t.startsWith("### ");
  });

  const pageMenu = [
    {
      label: "Edit",
      onClick: () => {
        setEditing(body || "# Personal\n\n");
        setDirty(false);
      },
    },
    {
      label: "Refresh summary",
      disabled: busy,
      onClick: () => void refreshOverview(),
    },
    { kind: "sep" as const },
    {
      label: "Copy all",
      onClick: () => {
        void copyText(body);
        onNotice?.("Copied personal page");
      },
    },
    {
      label: "Reveal in vault",
      onClick: () => void api.revealPath("personal.md").catch((e) => onError(errText(e))),
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
        <div className="dim tiny">
          Personal rollup{dirty ? " · unsaved" : ""}
        </div>
        <div className="grow" />
        {editing === null ? (
          <>
            <button className="btn" onClick={refreshOverview} disabled={busy}>
              {busy ? "Refreshing…" : "Refresh summary"}
            </button>
            <button
              className="btn"
              onClick={() => {
                setEditing(body || "# Personal\n\n");
                setDirty(false);
              }}
            >
              Edit
            </button>
          </>
        ) : (
          <>
            <button className="btn" onClick={cancel}>
              Cancel
            </button>
            <button className="btn primary" onClick={save}>
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
              setDirty(v !== body);
            }}
            vaultPath={vaultPath}
            onSave={() => void save()}
          />
        ) : hasContent ? (
          <Markdown
            text={body}
            vaultPath={vaultPath}
            onEdit={() => {
              setEditing(body);
              setDirty(false);
            }}
            extraMenu={pageMenu.filter((i) => i.kind === "sep" || i.label !== "Edit")}
          />
        ) : (
          <div className="empty" style={{ padding: 0 }}>
            <h2>No personal entries yet</h2>
            <p className="dim">
              When triage marks something as personal it shows up here. You can also Edit and write
              notes directly.
            </p>
            <p className="dim" style={{ marginTop: 16 }}>
              <button
                className="btn"
                onClick={() => {
                  setEditing("# Personal\n\n");
                  setDirty(false);
                }}
              >
                Start writing
              </button>
            </p>
          </div>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
