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

export default function IdeasView({ vaultPath, onError, onNotice }: Props) {
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const menu = useContextMenu();

  const refresh = useCallback(async () => {
    try {
      setBody(await api.readIdeas());
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
      await api.writeIdeas(editing);
      setBody(editing);
      setEditing(null);
      setDirty(false);
      onNotice?.("Saved");
    } catch (e) {
      onError(errText(e));
    }
  }

  const hasContent = body.split("\n").some((l) => {
    const t = l.trim();
    return t && !t.startsWith("#") && t !== "-";
  });

  const pageMenu = [
    {
      label: "Edit",
      onClick: () => {
        setEditing(body || "# Ideas\n\n");
        setDirty(false);
      },
    },
    { kind: "sep" as const },
    {
      label: "Copy all",
      onClick: () => {
        void copyText(body);
        onNotice?.("Copied ideas");
      },
    },
    {
      label: "Reveal in vault",
      onClick: () => void api.revealPath("ideas.md").catch((e) => onError(errText(e))),
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
        <div className="dim tiny">Ideas{dirty ? " · unsaved" : ""}</div>
        <div className="grow" />
        {editing === null ? (
          <button
            className="btn"
            onClick={() => {
              setEditing(body || "# Ideas\n\n");
              setDirty(false);
            }}
          >
            Edit
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
            <h2>No ideas yet</h2>
            <p className="dim">
              Maybe-someday thoughts land here after triage — or Edit and write them yourself.
            </p>
            <p className="dim" style={{ marginTop: 16 }}>
              <button
                className="btn"
                onClick={() => {
                  setEditing("# Ideas\n\n");
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
