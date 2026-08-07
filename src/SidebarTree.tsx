import { useCallback, useEffect, useMemo, useState } from "react";
import { api, errText, type Entry, type ProjectEntry } from "./api";
import { ContextMenu, useContextMenu } from "./ContextMenu";
import { useFormat } from "./FormatContext";
import { useNavigate } from "./nav";

type Props = {
  refreshTick?: number;
  activeSlug?: string | null;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

type Node = ProjectEntry & { children: Node[]; depth: number };

const OPEN_KEY = "daybook.tree.open";

function loadOpen(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(OPEN_KEY) ?? "[]") as string[]);
  } catch {
    return new Set();
  }
}

/**
 * Build the forest from `parent` slugs. Anything pointing at a page that no
 * longer exists is treated as top-level rather than dropped, so a bad parent
 * hides nothing.
 */
function buildTree(projects: ProjectEntry[]): Node[] {
  const bySlug = new Map<string, Node>();
  for (const p of projects) bySlug.set(p.slug, { ...p, children: [], depth: 0 });

  const roots: Node[] = [];
  for (const node of bySlug.values()) {
    const parent = node.parent ? bySlug.get(node.parent) : undefined;
    if (parent && parent.slug !== node.slug) parent.children.push(node);
    else roots.push(node);
  }

  const sort = (list: Node[], depth: number) => {
    list.sort((a, b) => a.name.localeCompare(b.name));
    for (const n of list) {
      n.depth = depth;
      sort(n.children, depth + 1);
    }
  };
  sort(roots, 0);
  return roots;
}

/** Notion's sidebar: the page tree, with what is actually inside each page. */
export default function SidebarTree({ refreshTick, activeSlug, onError, onNotice }: Props) {
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [entries, setEntries] = useState<Record<string, Entry[]>>({});
  const [open, setOpen] = useState<Set<string>>(loadOpen);
  const menu = useContextMenu();
  const navigate = useNavigate();
  const fmt = useFormat();

  const load = useCallback(async () => {
    try {
      setProjects(await api.listProjects());
    } catch (e) {
      onError(errText(e));
    }
  }, [onError]);

  useEffect(() => {
    load();
  }, [load, refreshTick]);

  useEffect(() => {
    localStorage.setItem(OPEN_KEY, JSON.stringify([...open]));
  }, [open]);

  // Children are fetched only when a node is opened; loading every project's
  // entries up front would query the whole vault to draw a sidebar.
  const loadEntries = useCallback(
    async (slug: string) => {
      if (entries[slug]) return;
      try {
        const found = await api.queryEntries({ slug, limit: 25 });
        setEntries((prev) => ({ ...prev, [slug]: found }));
      } catch {
        setEntries((prev) => ({ ...prev, [slug]: [] }));
      }
    },
    [entries]
  );

  function toggle(slug: string) {
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(slug)) next.delete(slug);
      else {
        next.add(slug);
        void loadEntries(slug);
      }
      return next;
    });
  }

  const tree = useMemo(() => buildTree(projects), [projects]);

  async function reparent(slug: string, kind: string, parent: string) {
    try {
      await api.setEntityParent(kind, slug, parent);
      setOpen((prev) => new Set(prev).add(parent));
      onNotice?.(parent ? "Nested" : "Moved to top level");
      await load();
    } catch (e) {
      onError(errText(e));
    }
  }

  function nodeMenu(n: Node) {
    const others = projects.filter((p) => p.slug !== n.slug);
    return [
      {
        label: "Open",
        onClick: () =>
          navigate({
            type: "entity",
            kind: n.kind === "area" ? "area" : "project",
            slug: n.slug,
          }),
      },
      { kind: "sep" as const },
      ...(n.parent
        ? [
            {
              label: "Move to top level",
              onClick: () => void reparent(n.slug, n.kind, ""),
            },
          ]
        : []),
      ...others.slice(0, 12).map((p) => ({
        label: `Nest under ${p.name}`,
        disabled: n.parent === p.slug,
        onClick: () => void reparent(n.slug, n.kind, p.slug),
      })),
    ];
  }

  function renderNode(n: Node): React.ReactNode {
    const isOpen = open.has(n.slug);
    const kids = entries[n.slug] ?? [];
    const hasChildren = n.children.length > 0 || kids.length > 0 || !isOpen;

    return (
      <div key={`${n.kind}:${n.slug}`}>
        <div
          className={`tree-row ${activeSlug === n.slug ? "active" : ""}`}
          style={{ paddingLeft: 6 + n.depth * 11 }}
          onContextMenu={(e) => menu.open(e, nodeMenu(n))}
        >
          <button
            className={`tree-caret ${isOpen ? "open" : ""}`}
            onClick={() => toggle(n.slug)}
            aria-label={isOpen ? "Collapse" : "Expand"}
            tabIndex={-1}
          >
            {hasChildren ? "▸" : ""}
          </button>
          <button
            className="tree-label"
            title={n.name}
            onClick={() =>
              navigate({
                type: "entity",
                kind: n.kind === "area" ? "area" : "project",
                slug: n.slug,
              })
            }
          >
            {n.name}
          </button>
          {n.status === "done" && <span className="tree-flag">✓</span>}
        </div>

        {isOpen && (
          <>
            {n.children.map(renderNode)}
            {kids.map((e) => (
              <div
                key={e.id}
                className="tree-row tree-entry"
                style={{ paddingLeft: 6 + (n.depth + 1) * 11 }}
              >
                <span className="tree-caret" />
                <button
                  className="tree-label dim"
                  title={`${e.title} · ${fmt.date(e.date)}`}
                  onClick={() =>
                    navigate({
                      type: "entity",
                      kind: n.kind === "area" ? "area" : "project",
                      slug: n.slug,
                    })
                  }
                >
                  {e.title || "(untitled)"}{" "}
                  <span className="tree-date">{fmt.date(e.date)}</span>
                </button>
              </div>
            ))}
            {isOpen && kids.length === 0 && n.children.length === 0 && (
              <div
                className="tree-row tree-empty dim"
                style={{ paddingLeft: 6 + (n.depth + 1) * 11 }}
              >
                empty
              </div>
            )}
          </>
        )}
      </div>
    );
  }

  if (projects.length === 0) return null;

  return (
    <div className="tree">
      {tree.map(renderNode)}
      <ContextMenu {...menu.menuProps} />
    </div>
  );
}
