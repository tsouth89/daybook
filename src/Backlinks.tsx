import { useEffect, useState } from "react";
import { api, errText, type Backlink } from "./api";
import { pathToNav, useNavigate } from "./nav";

/** Lightweight “Linked from” panel for Obsidian-style backlinks. */
export default function Backlinks({
  target,
  onError,
}: {
  /** Vault-relative target without .md, e.g. projects/daybook */
  target: string | null;
  onError?: (m: string) => void;
}) {
  const [links, setLinks] = useState<Backlink[]>([]);
  const navigate = useNavigate();

  useEffect(() => {
    if (!target) {
      setLinks([]);
      return;
    }
    let cancelled = false;
    api
      .listBacklinks(target)
      .then((l) => {
        if (!cancelled) setLinks(l);
      })
      .catch((e) => onError?.(errText(e)));
    return () => {
      cancelled = true;
    };
  }, [target, onError]);

  if (!target || !links.length) return null;

  return (
    <div className="backlinks">
      <h3 className="section-label">Linked from ({links.length})</h3>
      <ul className="backlink-list">
        {links.map((l, i) => (
          <li key={`${l.path}:${l.line}:${i}`}>
            <button
              type="button"
              className="linkbtn mono tiny"
              onClick={() => {
                const nav = pathToNav(l.path);
                if (nav) navigate(nav);
              }}
            >
              {l.path}
            </button>
            <span className="dim tiny">:{l.line}</span>
            <div className="dim tiny wrap">{l.text}</div>
          </li>
        ))}
      </ul>
    </div>
  );
}
