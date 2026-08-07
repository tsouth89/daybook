import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  errText,
  type Entry,
  type InboxProcessResult,
  type ProjectEntry,
} from "../api";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import EntryEditor, { blankEntry } from "../EntryEditor";
import { useFormat } from "../FormatContext";
import Markdown from "../Markdown";
import ProcessResult from "../ProcessResult";
import { useNavigate } from "../nav";
import { useViewHandlers } from "../viewhost";

type Props = {
  /** ISO date for today, from the backend clock. */
  date: string;
  refreshTick?: number;
  vaultPath: string;
  onChanged: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

/** ISO date `days` before `from`. */
function back(from: string, days: number): string {
  const d = new Date(`${from}T00:00:00`);
  d.setDate(d.getDate() - days);
  return d.toISOString().slice(0, 10);
}

/**
 * What you have been thinking about lately, cleaned up and readable.
 *
 * Not a dashboard. The point of the app is that you dump something and get
 * back prose worth reading, so this is a digest in reverse-chronological
 * order — no counts, no badges, no progress bars.
 */
export default function HomeView({
  date,
  refreshTick,
  vaultPath,
  onChanged,
  onError,
  onNotice,
}: Props) {
  const [entries, setEntries] = useState<Entry[]>([]);
  const [days, setDays] = useState(14);
  const [pending, setPending] = useState(0);
  const [busy, setBusy] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [scanned, setScanned] = useState(false);
  const [result, setResult] = useState<InboxProcessResult | null>(null);
  const [editing, setEditing] = useState<Entry | null>(null);
  // Only so the editor can re-home an entry; the feed itself shows no project list.
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const menu = useContextMenu();
  const fmt = useFormat();
  const navigate = useNavigate();

  const load = useCallback(async () => {
    try {
      const [found, inbox] = await Promise.all([
        api.queryEntries({ since: back(date, days), limit: 300 }),
        api.listInbox(),
      ]);
      setEntries(found);
      setPending(inbox.filter((i) => i.date === date).length);
      return found.length;
    } catch (e) {
      onError(errText(e));
      return 0;
    }
  }, [date, days, onError]);

  useEffect(() => {
    load();
  }, [load, refreshTick]);

  // First run against a vault that predates the index: recovering costs
  // nothing, so do it rather than showing an empty page and asking.
  useEffect(() => {
    if (scanned || scanning) return;
    let cancelled = false;
    (async () => {
      const n = await load();
      if (cancelled || n > 0) {
        setScanned(true);
        return;
      }
      setScanning(true);
      try {
        const report = await api.rebuildEntryIndex();
        if (!cancelled && report.recovered > 0) {
          onNotice?.(`Recovered ${report.recovered} entries`);
          await load();
        }
      } catch (e) {
        onError(errText(e));
      } finally {
        if (!cancelled) {
          setScanning(false);
          setScanned(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [scanned, scanning, load, onError, onNotice]);

  useEffect(() => {
    api.listProjects().then(setProjects).catch(() => setProjects([]));
  }, []);

  useEffect(() => {
    const onFocus = () => load();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load]);

  const process = useCallback(async () => {
    if (busy || pending === 0) return;
    setBusy(true);
    setResult(null);
    try {
      const r = await api.processDay(date);
      setResult(r);
      await load();
      onChanged();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }, [busy, pending, date, load, onChanged, onError]);

  useViewHandlers({ process: () => void process() });

  /** Reverse-chronological, grouped by day. */
  const byDay = useMemo(() => {
    const groups = new Map<string, Entry[]>();
    for (const e of entries) {
      const list = groups.get(e.date);
      if (list) list.push(e);
      else groups.set(e.date, [e]);
    }
    return [...groups.entries()]
      .sort((a, b) => b[0].localeCompare(a[0]))
      .map(([day, list]) => ({
        day,
        list: list.sort((a, b) => (b.time || "").localeCompare(a.time || "")),
      }));
  }, [entries]);

  function dayLabel(d: string): string {
    if (d === date) return "Today";
    if (d === back(date, 1)) return "Yesterday";
    return fmt.date(d);
  }

  function openOwner(e: Entry) {
    if (e.slug) {
      navigate({
        type: "entity",
        kind: e.kind === "area" ? "area" : "project",
        slug: e.slug,
      });
    } else {
      navigate({ type: "day", date: e.date, pane: "note" });
    }
  }

  function entryMenu(e: Entry) {
    return [
      { label: "Edit…", onClick: () => setEditing(e) },
      {
        label: e.slug ? `Open ${e.name || e.slug}` : "Open the day",
        onClick: () => openOwner(e),
      },
      {
        label: "Copy text",
        onClick: () => {
          void copyText(e.body || e.title);
          onNotice?.("Copied");
        },
      },
    ];
  }

  return (
    <div
      className="content feed"
      onContextMenu={(e) => {
        if ((e.target as HTMLElement).closest("button, input, .ctxmenu, .entry")) return;
        menu.open(e, [
          { label: "Capture…", onClick: () => void api.showCapture() },
          { label: "Add by hand…", onClick: () => setEditing(blankEntry(date)) },
          {
            label: `File what's waiting (${pending})`,
            disabled: busy || pending === 0,
            onClick: () => void process(),
          },
        ]);
      }}
    >
      <div className="feed-strip">
        <span className="feed-title">Lately</span>
        {pending > 0 && (
          <span className="strip-flag warn">
            {pending} waiting to be filed
          </span>
        )}
        <span className="grow" />
        <button className="btn" onClick={() => api.showCapture()}>
          Capture
        </button>
        {pending > 0 && (
          <button className="btn primary" onClick={() => void process()} disabled={busy}>
            {busy ? "Filing…" : "File them"}
          </button>
        )}
      </div>

      {result && <ProcessResult result={result} label="Filed" />}
      {scanning && <p className="dim">Reading your vault…</p>}

      {!scanning && byDay.length === 0 ? (
        <div className="empty" style={{ padding: "24px 0" }}>
          <h2>Nothing here yet</h2>
          <p className="dim">
            Hit <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd> and say whatever is on your
            mind. Daybook cleans it up and files it, and it shows up here to read.
          </p>
          <p style={{ marginTop: 16 }}>
            <button className="btn primary" onClick={() => api.showCapture()}>
              Capture
            </button>
          </p>
        </div>
      ) : (
        <>
          {byDay.map(({ day, list }) => (
            <section key={day} className="feed-day">
              <h2 className="feed-day-head">{dayLabel(day)}</h2>
              {list.map((e) => (
                <article
                  key={e.id}
                  className="entry"
                  onDoubleClick={() => setEditing(e)}
                  onContextMenu={(ev) => {
                    ev.stopPropagation();
                    menu.open(ev, entryMenu(e));
                  }}
                >
                  <div className="entry-meta">
                    {e.slug ? (
                      <button className="entry-owner" onClick={() => openOwner(e)}>
                        {e.name || e.slug}
                      </button>
                    ) : (
                      <span className="entry-owner plain">
                        {e.kind === "idea" ? "Idea" : e.kind === "task" ? "Task" : "Note"}
                      </span>
                    )}
                    {e.time && <span className="dim tiny">{fmt.time(e.time)}</span>}
                  </div>

                  {e.title && <h3 className="entry-title">{e.title}</h3>}

                  {e.body && (
                    <div className="entry-body">
                      <Markdown text={e.body} vaultPath={vaultPath} />
                    </div>
                  )}

                  {(e.decisions.length > 0 || e.open.length > 0) && (
                    <div className="entry-extra">
                      {e.decisions.map((d, i) => (
                        <div key={`d${i}`}>
                          <span className="entry-tag">Decided</span> {d}
                        </div>
                      ))}
                      {e.open.map((o, i) => (
                        <div key={`o${i}`}>
                          <span className="entry-tag open">Open</span> {o}
                        </div>
                      ))}
                    </div>
                  )}
                </article>
              ))}
            </section>
          ))}

          <div className="feed-more">
            <button className="btn" onClick={() => setDays((d) => d + 30)}>
              Show earlier
            </button>
          </div>
        </>
      )}

      <ContextMenu {...menu.menuProps} />
      <EntryEditor
        entry={editing}
        projects={projects}
        onClose={() => setEditing(null)}
        onSaved={() => void load()}
        onError={onError}
        onNotice={onNotice}
      />
    </div>
  );
}
