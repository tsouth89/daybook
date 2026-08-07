import { useCallback, useEffect, useState } from "react";
import {
  api,
  errText,
  type Entry,
  type ProjectEntry,
  type SearchHit,
} from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import { useFormat } from "../FormatContext";
import { pathToNav, useNavigate } from "../nav";

type Scope = "" | "personal" | "work";
type Kind = "" | "project" | "area" | "task" | "idea" | "note";

/**
 * Two ways to find things, because the vault has two halves. Filters query the
 * item layer by property; the text below is the old grep over raw markdown,
 * which still catches anything hand-written that was never routed.
 */
export default function SearchView({
  onError,
  onNotice,
}: {
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<Scope>("");
  const [kind, setKind] = useState<Kind>("");
  const [slug, setSlug] = useState("");
  const [openOnly, setOpenOnly] = useState(false);
  const [entries, setEntries] = useState<Entry[]>([]);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [ran, setRan] = useState(false);
  const menu = useContextMenu();
  const navigate = useNavigate();
  const fmt = useFormat();

  useEffect(() => {
    api.listProjects().then(setProjects).catch(() => setProjects([]));
  }, []);

  const hasFilter = !!(scope || kind || slug || openOnly);

  const run = useCallback(async () => {
    const q = query.trim();
    if (!q && !hasFilter) {
      setEntries([]);
      setHits([]);
      setRan(false);
      return;
    }
    try {
      const [found, textHits] = await Promise.all([
        api.queryEntries({
          text: q || undefined,
          scope: scope || undefined,
          kind: kind || undefined,
          slug: slug || undefined,
          open_only: openOnly || undefined,
          limit: 200,
        }),
        // Filters describe the item layer, so a filtered search stays there.
        q && !hasFilter ? api.search(q) : Promise.resolve([] as SearchHit[]),
      ]);
      setEntries(found);
      setHits(textHits);
      setRan(true);
    } catch (e) {
      onError(errText(e));
    }
  }, [query, scope, kind, slug, openOnly, hasFilter, onError]);

  useEffect(() => {
    const t = setTimeout(run, 200);
    return () => clearTimeout(t);
  }, [run]);

  function openPath(path: string) {
    const nav = pathToNav(path);
    if (nav) navigate(nav);
    else onError(`Can't open ${path} in-app yet — use Reveal in vault.`);
  }

  function openEntry(e: Entry) {
    if (e.slug) {
      const k = projects.find((p) => p.slug === e.slug)?.kind === "area" ? "area" : "project";
      navigate({ type: "entity", kind: k, slug: e.slug });
    } else {
      navigate({ type: "day", date: e.date, pane: "note" });
    }
  }

  const grouped = hits.reduce<Record<string, SearchHit[]>>((acc, h) => {
    (acc[h.path] ??= []).push(h);
    return acc;
  }, {});

  function chip<T extends string>(
    label: string,
    value: T,
    current: T,
    set: (v: T) => void
  ) {
    return (
      <button
        key={label}
        className={`tab ${current === value ? "active" : ""}`}
        onClick={() => set(current === value ? ("" as T) : value)}
      >
        {label}
      </button>
    );
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

      <div className="filterbar">
        <span className="dim tiny">Scope</span>
        {chip("personal", "personal" as Scope, scope, setScope)}
        {chip("work", "work" as Scope, scope, setScope)}
        <span className="dim tiny">Kind</span>
        {chip("project", "project" as Kind, kind, setKind)}
        {chip("area", "area" as Kind, kind, setKind)}
        {chip("task", "task" as Kind, kind, setKind)}
        {chip("idea", "idea" as Kind, kind, setKind)}
        {chip("note", "note" as Kind, kind, setKind)}
        <button
          className={`tab ${openOnly ? "active" : ""}`}
          onClick={() => setOpenOnly((v) => !v)}
        >
          open loops
        </button>
        <select
          className="filterselect"
          value={slug}
          onChange={(e) => setSlug(e.target.value)}
        >
          <option value="">any project</option>
          {projects.map((p) => (
            <option key={`${p.kind}:${p.slug}`} value={p.slug}>
              {p.name}
            </option>
          ))}
        </select>
        {(hasFilter || query) && (
          <button
            className="linkbtn tiny"
            onClick={() => {
              setQuery("");
              setScope("");
              setKind("");
              setSlug("");
              setOpenOnly(false);
            }}
          >
            clear
          </button>
        )}
      </div>

      {ran && (
        <div className="dim tiny pad">
          {entries.length} entr{entries.length === 1 ? "y" : "ies"}
          {hits.length > 0 && ` · ${hits.length} more in raw text`}
        </div>
      )}

      <div className="content">
        {entries.length > 0 && (
          <div className="entry-results">
            {entries.map((e) => (
              <div
                key={e.id}
                className="entry-hit"
                onDoubleClick={() => openEntry(e)}
                onContextMenu={(ev) =>
                  menu.open(ev, [
                    { label: "Open", onClick: () => openEntry(e) },
                    {
                      label: "Open the day",
                      onClick: () => navigate({ type: "day", date: e.date, pane: "note" }),
                    },
                    {
                      label: "Copy text",
                      onClick: () => {
                        void copyText(e.body || e.title);
                        onNotice?.("Copied");
                      },
                    },
                  ])
                }
              >
                <div className="entry-hit-head">
                  <button className="linkbtn" onClick={() => openEntry(e)}>
                    {e.title || "(untitled)"}
                  </button>
                  <span className="pill">{e.kind}</span>
                  <span className="pill">{e.scope}</span>
                  {e.slug && <span className="chip">{e.name || e.slug}</span>}
                  {e.kind === "task" && e.done && <span className="pill ok">done</span>}
                  {e.due && <span className="dim tiny">due {fmt.date(e.due)}</span>}
                  <span className="grow" />
                  <span className="dim tiny mono">{fmt.date(e.date)}</span>
                </div>
                {e.body && <div className="entry-hit-body dim">{e.body.slice(0, 240)}</div>}
                {e.open.length > 0 && (
                  <ul className="loop-list">
                    {e.open.map((o, i) => (
                      <li key={i}>{o}</li>
                    ))}
                  </ul>
                )}
              </div>
            ))}
          </div>
        )}

        {Object.keys(grouped).length > 0 && (
          <>
            <h3 className="section-label pad-top">In the raw text</h3>
            {Object.entries(grouped).map(([path, group]) => (
              <div
                key={path}
                className="hitgroup"
                onContextMenu={(e) => {
                  if ((e.target as HTMLElement).closest(".hit")) return;
                  menu.open(e, [
                    { label: "Open in app", onClick: () => openPath(path) },
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
                  ]);
                }}
              >
                <button
                  type="button"
                  className="hitpath mono linkish"
                  onClick={() => openPath(path)}
                >
                  {path}
                </button>
                {group.map((h, i) => (
                  <div key={i} className="hit" onDoubleClick={() => openPath(path)}>
                    <span className="dim mono tiny">{h.line}</span> {h.text}
                  </div>
                ))}
              </div>
            ))}
          </>
        )}

        {ran && entries.length === 0 && hits.length === 0 && (
          <p className="dim tiny">Nothing matched.</p>
        )}
      </div>
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
