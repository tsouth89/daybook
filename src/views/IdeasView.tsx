import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

export default function IdeasView({ vaultPath, onError, onNotice }: Props) {
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
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
      onClick: () => setEditing(body || "# Ideas\n\n"),
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
        if ((e.target as HTMLElement).closest(".md, textarea, .ctxmenu")) return;
        menu.open(e, pageMenu);
      }}
    >
      <div className="toolbar">
        <div className="dim tiny">Ideas</div>
        <div className="grow" />
        {editing === null ? (
          <button className="btn" onClick={() => setEditing(body || "# Ideas\n\n")}>
            Edit
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
      <div className="content">
        {editing !== null ? (
          <textarea
            className="rawedit"
            value={editing}
            onChange={(e) => setEditing(e.target.value)}
            spellCheck={false}
          />
        ) : hasContent ? (
          <Markdown
            text={body}
            vaultPath={vaultPath}
            onEdit={() => setEditing(body)}
            extraMenu={pageMenu.filter((i) => i.kind === "sep" || i.label !== "Edit")}
          />
        ) : (
          <div className="empty" style={{ padding: 0 }}>
            <h2>No ideas yet</h2>
            <p className="dim">
              Maybe-someday thoughts land here after triage — or Edit and write them yourself.
            </p>
            <p className="dim" style={{ marginTop: 16 }}>
              <button className="btn" onClick={() => setEditing("# Ideas\n\n")}>
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
