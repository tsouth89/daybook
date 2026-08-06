import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
};

export default function PersonalView({ vaultPath, onError }: Props) {
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
    } catch (e) {
      onError(errText(e));
    }
  }

  async function refreshOverview() {
    setBusy(true);
    try {
      const next = await api.refreshPersonalOverview();
      setBody(next);
      if (editing !== null) setEditing(next);
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

  return (
    <div className="detail" style={{ flex: 1 }}>
      <div className="toolbar">
        <div className="dim tiny">Personal rollup</div>
        <div className="grow" />
        {editing === null ? (
          <>
            <button className="btn" onClick={refreshOverview} disabled={busy}>
              {busy ? "Refreshing…" : "Refresh summary"}
            </button>
            <button className="btn" onClick={() => setEditing(body || "# Personal\n\n")}>
              Edit
            </button>
          </>
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
          <Markdown text={body} vaultPath={vaultPath} />
        ) : (
          <div className="empty" style={{ padding: 0 }}>
            <h2>No personal entries yet</h2>
            <p className="dim">
              When triage marks something as personal it shows up here. You can also Edit and write
              notes directly.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
