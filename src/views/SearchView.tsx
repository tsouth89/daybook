import { useEffect, useState } from "react";
import { api, errText, type SearchHit } from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { pathToNav, useNavigate } from "../nav";

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
  const navigate = useNavigate();

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

  function openPath(path: string) {
    const nav = pathToNav(path);
    if (nav) navigate(nav);
    else onError(`Can't open ${path} in-app yet — use Reveal in vault.`);
  }

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
          <span className="dim"> · click a path to open</span>
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
                  label: "Open in app",
                  onClick: () => openPath(path),
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
                  onClick: () => void api.revealPath(path).catch((err) => onError(errText(err))),
                },
              ]);
            }}
          >
            <button type="button" className="hitpath mono linkish" onClick={() => openPath(path)}>
              {path}
            </button>
            {group.map((h, i) => (
              <div
                key={i}
                className="hit"
                onDoubleClick={() => openPath(path)}
                onContextMenu={(e) =>
                  menu.open(e, [
                    {
                      label: "Open in app",
                      onClick: () => openPath(path),
                    },
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
