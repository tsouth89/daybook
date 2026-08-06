import { useCallback, useEffect, useState } from "react";
import { api, errText } from "../api";
import Markdown from "../Markdown";

type Props = {
  vaultPath: string;
  onError: (m: string) => void;
};

export default function IdeasView({ vaultPath, onError }: Props) {
  const [body, setBody] = useState("");

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
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  const hasContent = body.split("\n").some((l) => {
    const t = l.trim();
    return t && !t.startsWith("#") && t !== "-";
  });

  if (!hasContent) {
    return (
      <div className="empty">
        <h2>No ideas yet</h2>
        <p className="dim">
          Maybe-someday thoughts land here after triage — side projects, random what-ifs, things
          worth revisiting.
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
