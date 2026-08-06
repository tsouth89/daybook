import { useCallback, useEffect, useState } from "react";
import { api, errText, type ProjectEntry, type ProjectMeta } from "../api";
import Markdown from "../Markdown";

export default function ProjectsView({
  vaultPath,
  onError,
}: {
  vaultPath: string;
  onError: (m: string) => void;
}) {
  const [projects, setProjects] = useState<ProjectEntry[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [body, setBody] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [newKind, setNewKind] = useState<"project" | "area">("project");
  const [newScope, setNewScope] = useState<"personal" | "work">("personal");
  const [filter, setFilter] = useState<"all" | "project" | "area" | "personal" | "work">(
    "all"
  );

  const refreshList = useCallback(async () => {
    try {
      setProjects(await api.listProjects());
    } catch (e) {
      onError(errText(e));
    }
  }, [onError]);

  useEffect(() => {
    refreshList();
  }, [refreshList]);

  const visible = projects.filter((p) => {
    if (filter === "all") return true;
    if (filter === "project" || filter === "area") return p.kind === filter;
    return p.scope === filter;
  });

  useEffect(() => {
    if (!selected && visible.length) setSelected(`${visible[0].kind}:${visible[0].slug}`);
  }, [visible, selected]);

  useEffect(() => {
    if (!selected) return;
    setEditing(null);
    const [kind, slug] = selected.split(":");
    api.readEntity(kind, slug).then(setBody).catch((e) => onError(errText(e)));
  }, [selected, onError]);

  async function save() {
    if (!selected || editing === null) return;
    const [kind, slug] = selected.split(":");
    try {
      await api.writeEntity(kind, slug, editing);
      setBody(editing);
      setEditing(null);
      await refreshList();
    } catch (e) {
      onError(errText(e));
    }
  }

  async function refreshOverview() {
    if (!selected) return;
    const [kind, slug] = selected.split(":");
    setBusy(true);
    try {
      const next = await api.refreshEntityOverview(kind, slug);
      setBody(next);
      if (editing !== null) setEditing(next);
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function create() {
    if (!newName.trim()) return;
    try {
      const meta: ProjectMeta = await api.createEntity(newKind, newName.trim(), newScope);
      setCreating(false);
      setNewName("");
      await refreshList();
      setSelected(`${meta.kind}:${meta.slug}`);
    } catch (e) {
      onError(errText(e));
    }
  }

  function onListContextMenu(e: React.MouseEvent) {
    e.preventDefault();
    setCreating(true);
  }

  return (
    <div className="split">
      <div className="list" onContextMenu={onListContextMenu}>
        <div className="list-head filters">
          {(["all", "project", "area", "work", "personal"] as const).map((f) => (
            <button
              key={f}
              className={`tab ${filter === f ? "active" : ""}`}
              onClick={() => {
                setFilter(f);
                setSelected(null);
              }}
            >
              {f}
            </button>
          ))}
        </div>
        <div className="list-head">
          <button className="btn tiny-btn" onClick={() => setCreating(true)}>
            New project / area
          </button>
        </div>
        {creating && (
          <div className="createbox">
            <input
              autoFocus
              placeholder="Name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") create();
                if (e.key === "Escape") setCreating(false);
              }}
            />
            <div className="row gap">
              <select
                value={newKind}
                onChange={(e) => setNewKind(e.target.value as "project" | "area")}
              >
                <option value="project">project</option>
                <option value="area">area</option>
              </select>
              <select
                value={newScope}
                onChange={(e) => setNewScope(e.target.value as "personal" | "work")}
              >
                <option value="personal">personal</option>
                <option value="work">work</option>
              </select>
            </div>
            <div className="row gap">
              <button className="btn" onClick={() => setCreating(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={create} disabled={!newName.trim()}>
                Create
              </button>
            </div>
          </div>
        )}
        {visible.map((p) => {
          const key = `${p.kind}:${p.slug}`;
          return (
            <button
              key={key}
              className={`listitem ${selected === key ? "active" : ""}`}
              onClick={() => setSelected(key)}
              onContextMenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setCreating(true);
                setNewKind(p.kind === "area" ? "area" : "project");
                setNewScope(p.scope === "work" ? "work" : "personal");
              }}
            >
              <div className="row">
                <span>{p.name}</span>
                <span className="pill">
                  {p.kind} · {p.scope}
                </span>
              </div>
              <div className="preview dim mono">
                last: {p.last_date || "—"} · {p.day_count}d
              </div>
            </button>
          );
        })}
        {!projects.length && !creating && (
          <p className="dim tiny" style={{ padding: "12px" }}>
            Right-click or use New to create a project. They also appear when inbox items are
            processed.
          </p>
        )}
        {projects.length > 0 && visible.length === 0 && (
          <p className="dim tiny" style={{ padding: "12px" }}>
            Nothing in this filter.
          </p>
        )}
      </div>
      <div className="detail">
        {selected ? (
          <>
            <div className="toolbar">
              <div className="grow" />
              {editing === null ? (
                <>
                  <button className="btn" onClick={refreshOverview} disabled={busy}>
                    {busy ? "Refreshing…" : "Refresh summary"}
                  </button>
                  <button className="btn" onClick={() => setEditing(body)}>
                    Edit
                  </button>
                </>
              ) : (
                <>
                  <button className="btn" onClick={() => setEditing(null)}>
                    Cancel
                  </button>
                  <button className="btn primary" onClick={save}>
                    Save
                  </button>
                </>
              )}
            </div>
            <div className="content">
              {editing !== null ? (
                <textarea
                  className="rawedit"
                  value={editing}
                  onChange={(e) => setEditing(e.target.value)}
                  spellCheck={false}
                />
              ) : (
                <Markdown text={body} vaultPath={vaultPath} />
              )}
            </div>
          </>
        ) : (
          <div className="empty">
            <h2>Projects & areas</h2>
            <p className="dim">
              Pick one from the list, or create a new project/area. Each page has a standing
              Overview plus a dated activity log.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
