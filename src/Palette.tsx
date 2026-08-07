import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api, type DayEntry, type Entry, type ProjectEntry } from "./api";
import { useFormat } from "./FormatContext";
import type { NavTarget } from "./nav";

export type Command = { id: string; label: string; hint?: string; run: () => void };

type Item = {
  key: string;
  label: string;
  detail: string;
  group: "Go" | "Projects" | "Days" | "Entries" | "Actions";
  run: () => void;
};

/**
 * Subsequence match, the cheap fuzzy people actually expect: "dbk" finds
 * "Daybook". Score favours earlier and tighter matches.
 */
function fuzzy(needle: string, hay: string): number | null {
  if (!needle) return 0;
  const n = needle.toLowerCase();
  const h = hay.toLowerCase();
  if (h.includes(n)) return 1000 - h.indexOf(n);

  let score = 0;
  let at = -1;
  for (const ch of n) {
    const next = h.indexOf(ch, at + 1);
    if (next === -1) return null;
    score += next === at + 1 ? 5 : 1;
    at = next;
  }
  return score;
}

type Props = {
  open: boolean;
  onClose: () => void;
  navigate: (t: NavTarget) => void;
  commands: Command[];
};

/** Ctrl+K. The thing that turns a set of tabs into something you can drive. */
export default function Palette({ open, onClose, navigate, commands }: Props) {
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [days, setDays] = useState<DayEntry[]>([]);
  const [entries, setEntries] = useState<Entry[]>([]);
  const listRef = useRef<HTMLDivElement>(null);
  const fmt = useFormat();

  useEffect(() => {
    if (!open) return;
    setQuery("");
    setCursor(0);
    api.listProjects().then(setProjects).catch(() => setProjects([]));
    api.listDays().then(setDays).catch(() => setDays([]));
  }, [open]);

  // Entries are searched server-side; the others are small enough to hold.
  useEffect(() => {
    if (!open || query.trim().length < 2) {
      setEntries([]);
      return;
    }
    let cancelled = false;
    const t = setTimeout(() => {
      api
        .queryEntries({ text: query.trim(), limit: 8 })
        .then((e) => !cancelled && setEntries(e))
        .catch(() => setEntries([]));
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [open, query]);

  const items = useMemo<Item[]>(() => {
    const out: Item[] = [];
    for (const c of commands) {
      out.push({
        key: `cmd:${c.id}`,
        label: c.label,
        detail: c.hint ?? "",
        group: c.label.startsWith("Go to") ? "Go" : "Actions",
        run: c.run,
      });
    }
    for (const p of projects) {
      out.push({
        key: `proj:${p.kind}:${p.slug}`,
        label: p.name,
        detail: `${p.kind}${p.status && p.status !== "active" ? ` · ${p.status}` : ""}`,
        group: "Projects",
        run: () =>
          navigate({
            type: "entity",
            kind: p.kind === "area" ? "area" : "project",
            slug: p.slug,
          }),
      });
    }
    for (const d of days) {
      out.push({
        key: `day:${d.date}`,
        label: fmt.date(d.date),
        detail: d.preview.slice(0, 60),
        group: "Days",
        run: () => navigate({ type: "day", date: d.date, pane: "note" }),
      });
    }
    for (const e of entries) {
      out.push({
        key: `entry:${e.id}`,
        label: e.title || "(untitled)",
        detail: `${e.kind}${e.name ? ` · ${e.name}` : ""} · ${fmt.date(e.date)}`,
        group: "Entries",
        run: () =>
          e.slug
            ? navigate({
                type: "entity",
                kind: e.kind === "area" ? "area" : "project",
                slug: e.slug,
              })
            : navigate({ type: "day", date: e.date, pane: "note" }),
      });
    }
    return out;
  }, [commands, projects, days, entries, navigate, fmt]);

  const shown = useMemo(() => {
    const q = query.trim();
    const scored = items
      .map((it) => ({ it, score: fuzzy(q, `${it.label} ${it.detail}`) }))
      .filter((x) => x.score !== null) as { it: Item; score: number }[];
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, 40).map((x) => x.it);
  }, [items, query]);

  useEffect(() => setCursor(0), [query]);

  useEffect(() => {
    listRef.current
      ?.querySelector(`[data-idx="${cursor}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [cursor]);

  if (!open) return null;

  function choose(i: number) {
    const it = shown[i];
    if (!it) return;
    onClose();
    it.run();
  }

  let lastGroup = "";

  return createPortal(
    <div className="modal-backdrop palette-backdrop" onMouseDown={onClose}>
      <div className="palette" onMouseDown={(e) => e.stopPropagation()} role="dialog">
        <input
          autoFocus
          className="palette-input"
          placeholder="Jump to a project, day, entry, or action…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onClose();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setCursor((c) => Math.min(c + 1, shown.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setCursor((c) => Math.max(c - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              choose(cursor);
            }
          }}
          spellCheck={false}
        />
        <div className="palette-list" ref={listRef}>
          {shown.length === 0 && <div className="palette-empty dim tiny">No matches.</div>}
          {shown.map((it, i) => {
            const header = it.group !== lastGroup ? it.group : null;
            lastGroup = it.group;
            return (
              <div key={it.key}>
                {header && <div className="palette-group">{header}</div>}
                <button
                  data-idx={i}
                  className={`palette-row ${i === cursor ? "active" : ""}`}
                  onMouseMove={() => setCursor(i)}
                  onClick={() => choose(i)}
                >
                  <span className="palette-label">{it.label}</span>
                  {it.detail && <span className="dim tiny palette-detail">{it.detail}</span>}
                </button>
              </div>
            );
          })}
        </div>
        <div className="palette-foot dim tiny">
          <kbd>↑</kbd>
          <kbd>↓</kbd> move · <kbd>Enter</kbd> open · <kbd>Esc</kbd> close
        </div>
      </div>
    </div>,
    document.body
  );
}
