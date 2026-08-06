import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
};

export default function PersonalView({ vaultPath, onError }: Props) {
  const [body, setBody] = useState("");

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
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  const hasContent = body.split("\n").some((l) => {
    const t = l.trim();
    return t.startsWith("## ") || t.startsWith("### ");
  });

  if (!hasContent) {
    return (
      <div className="empty">
        <h2>No personal entries yet</h2>
        <p className="dim">
          When triage marks something as personal — a life note, a personal project update, a
          reminder — it shows up here as a rolling log over time. Work-scoped items stay out.
        </p>
      </div>
    );
  }

  return (
    <div className="content">
      <Markdown text={body} vaultPath={vaultPath} />
    </div>
  );
}
