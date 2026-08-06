import { useEffect, useState } from "react";
import { api, errText, type SearchHit } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";

export default function SearchView({
  onError,
  onNotice,
}: {
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [ran, setRan] = useState(false);
  const menu = useContextMenu();

  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      setRan(false);
      return;
    }
    const t = setTimeout(() => {
      api
        .search(query)
        .then((h) => {
          setHits(h);
          setRan(true);
        })
        .catch((e) => onError(errText(e)));
    }, 200);
    return () => clearTimeout(t);
  }, [query, onError]);

  const grouped = hits.reduce<Record<string, SearchHit[]>>((acc, h) => {
    (acc[h.path] ??= []).push(h);
    return acc;
  }, {});

  return (
    <div className="searchview">
      <input
        className="searchbox"
        autoFocus
        placeholder="Search everything…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        spellCheck={false}
      />
      {ran && (
        <div className="dim tiny pad">
          {hits.length} match{hits.length === 1 ? "" : "es"} in {Object.keys(grouped).length} file
          {Object.keys(grouped).length === 1 ? "" : "s"}
        </div>
      )}
      <div className="content">
        {Object.entries(grouped).map(([path, group]) => (
          <div
            key={path}
            className="hitgroup"
            onContextMenu={(e) => {
              if ((e.target as HTMLElement).closest(".hit")) return;
              menu.open(e, [
                {
                  label: "Copy path",
                  onClick: () => {
                    void copyText(path);
                    onNotice?.("Copied path");
                  },
                },
                {
                  label: "Reveal in vault",
                  onClick: () => void api.revealPath(path).catch((err) => onError(errText(err))),
                },
              ]);
            }}
          >
            <div className="hitpath mono">{path}</div>
            {group.map((h, i) => (
              <div
                key={i}
                className="hit"
                onContextMenu={(e) =>
                  menu.open(e, [
                    {
                      label: "Copy line",
                      onClick: () => {
                        void copyText(h.text);
                        onNotice?.("Copied line");
                      },
                    },
                    {
                      label: "Copy path",
                      onClick: () => {
                        void copyText(path);
                        onNotice?.("Copied path");
                      },
                    },
                    {
                      label: "Reveal in vault",
                      onClick: () =>
                        void api.revealPath(path).catch((err) => onError(errText(err))),
                    },
                  ])
                }
              >
                <span className="dim mono tiny">{h.line}</span> {h.text}
              </div>
            ))}
          </div>
        ))}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
