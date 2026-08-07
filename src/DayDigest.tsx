import { useCallback, useEffect, useState } from "react";
import { api, errText, type Entry } from "./api";
import { useFormat } from "./FormatContext";
import { useNavigate } from "./nav";

type Props = {
  date: string;
  refreshTick?: number;
  onError: (m: string) => void;
};

type Group = { slug: string; name: string; entries: Entry[] };

/**
 * What happened on a day, assembled from records rather than written by a
 * model. Triage already extracted `accomplished` / `decisions` / `open` for
 * every entry, so a summary is a query — free, and current the instant
 * anything is filed. Prose narration is the only part that needs a call, and
 * that stays a button.
 */
export default function DayDigest({ date, refreshTick, onError }: Props) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    try {
      setEntries(await api.queryEntries({ date, limit: 200 }));
    } catch (e) {
      onError(errText(e));
    }
  }, [date, onError]);

  useEffect(() => {
    load();
  }, [load, refreshTick]);

  if (entries.length === 0) return null;

  const groups: Group[] = [];
  for (const e of entries) {
    const key = e.slug || "";
    const g = groups.find((x) => x.slug === key);
    if (g) g.entries.push(e);
    else groups.push({ slug: key, name: e.name || key, entries: [e] });
  }
  groups.sort((a, b) => (a.slug ? 0 : 1) - (b.slug ? 0 : 1));

  const tasks = entries.filter((e) => e.kind === "task");
  const ideas = entries.filter((e) => e.kind === "idea");
  const decided = entries.flatMap((e) => e.decisions);
  const open = entries.flatMap((e) => e.open);

  function openProject(slug: string) {
    if (slug) navigate({ type: "entity", kind: "project", slug });
  }

  return (
    <div className="digest">
      <div className="digest-head">
        <h3 className="section-label">The day so far</h3>
        <span className="dim tiny">
          {entries.length} entr{entries.length === 1 ? "y" : "ies"} · updates as things file
        </span>
      </div>

      <div className="digest-cols">
        <div>
          {groups.map((g) => (
            <div key={g.slug || "_none"} className="digest-group">
              <div className="digest-group-head">
                {g.slug ? (
                  <button className="linkbtn" onClick={() => openProject(g.slug)}>
                    {g.name || g.slug}
                  </button>
                ) : (
                  <span className="dim">Unfiled</span>
                )}
              </div>
              <ul className="digest-list">
                {g.entries.map((e) => (
                  <li key={e.id}>
                    <span className="digest-time mono dim tiny">{fmt.time(e.time)}</span>
                    <span>{e.title || "(untitled)"}</span>
                    {e.accomplished.length > 0 && (
                      <ul className="digest-sub">
                        {e.accomplished.map((a, i) => (
                          <li key={i}>{a}</li>
                        ))}
                      </ul>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="digest-side">
          {decided.length > 0 && (
            <div>
              <h4 className="section-label">Decided</h4>
              <ul className="digest-list">
                {decided.map((d, i) => (
                  <li key={i}>{d}</li>
                ))}
              </ul>
            </div>
          )}
          {open.length > 0 && (
            <div>
              <h4 className="section-label">Left open</h4>
              <ul className="digest-list">
                {open.map((o, i) => (
                  <li key={i}>{o}</li>
                ))}
              </ul>
            </div>
          )}
          {tasks.length > 0 && (
            <div>
              <h4 className="section-label">Tasks raised ({tasks.length})</h4>
              <ul className="digest-list">
                {tasks.map((t) => (
                  <li key={t.id} className={t.done ? "struck" : ""}>
                    {t.title}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {ideas.length > 0 && (
            <div>
              <h4 className="section-label">Ideas ({ideas.length})</h4>
              <ul className="digest-list">
                {ideas.map((t) => (
                  <li key={t.id}>{t.title}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
