import { useCallback, useEffect, useState } from "react";
import { api, errText, type ProjectEntry, type ProjectMeta } from "../api";
import ConfirmDialog from "../ConfirmDialog";
import { ContextMenu, copyText, useContextMenu } from "../ContextMenu";
import Markdown from "../Markdown";
import NoteEditor from "../NoteEditor";

export default function ProjectsView({
  vaultPath,
  focusKey,
  onFocusConsumed,
  onError,
  onNotice,
}: {
  vaultPath: string;
  focusKey?: string | null;
  onFocusConsumed?: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
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
  const [confirmDelete, setConfirmDelete] = useState<{ kind: string; slug: string; name: string } | null>(
    null
  );
  const [dirty, setDirty] = useState(false);
  const menu = useContextMenu();

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

  useEffect(() => {
    if (!focusKey) return;
    setSelected(focusKey);
    onFocusConsumed?.();
  }, [focusKey, onFocusConsumed]);

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
    setDirty(false);
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
      setDirty(false);
      onNotice?.("Saved");
      await refreshList();
    } catch (e) {
      onError(errText(e));
    }
  }

  async function refreshOverview(kind?: string, slug?: string) {
    const key = selected;
    const k = kind ?? key?.split(":")[0];
    const s = slug ?? key?.split(":")[1];
    if (!k || !s) return;
    setBusy(true);
    try {
      const next = await api.refreshEntityOverview(k, s);
      if (selected === `${k}:${s}`) {
        setBody(next);
        if (editing !== null) setEditing(next);
      }
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

  async function doDelete() {
    if (!confirmDelete) return;
    const { kind, slug } = confirmDelete;
    try {
      await api.deleteEntity(kind, slug);
      setConfirmDelete(null);
      if (selected === `${kind}:${slug}`) {
        setSelected(null);
        setBody("");
      }
      await refreshList();
    } catch (e) {
      onError(errText(e));
    }
  }

  function relPath(kind: string, slug: string) {
    return `${kind === "area" ? "areas" : "projects"}/${slug}.md`;
  }

  function projectMenu(p: ProjectEntry) {
    const key = `${p.kind}:${p.slug}`;
    return [
      {
        label: "Open",
        onClick: () => setSelected(key),
      },
      {
        label: "Edit",
        onClick: () => {
          setSelected(key);
          api
            .readEntity(p.kind, p.slug)
            .then((t) => setEditing(t))
            .catch((e) => onError(errText(e)));
        },
      },
      {
        label: "Refresh summary",
        disabled: busy,
        onClick: () => {
          setSelected(key);
          void refreshOverview(p.kind, p.slug);
        },
      },
      { kind: "sep" as const },
      {
        label: "Copy name",
        onClick: () => {
          void copyText(p.name);
          onNotice?.("Copied name");
        },
      },
      {
        label: "Copy path",
        onClick: () => {
          void copyText(relPath(p.kind, p.slug));
          onNotice?.("Copied path");
        },
      },
      {
        label: "Reveal in vault",
        onClick: () =>
          void api.revealPath(relPath(p.kind, p.slug)).catch((e) => onError(errText(e))),
      },
      { kind: "sep" as const },
      {
        label: "New project / area…",
        onClick: () => {
          setNewKind(p.kind === "area" ? "area" : "project");
          setNewScope(p.scope === "work" ? "work" : "personal");
          setCreating(true);
        },
      },
      {
        label: `Delete ${p.kind}`,
        danger: true,
        onClick: () => setConfirmDelete({ kind: p.kind, slug: p.slug, name: p.name }),
      },
    ];
  }

  return (
    <div className="split">
      <div
        className="list"
        onContextMenu={(e) => {
          if ((e.target as HTMLElement).closest(".listitem")) return;
          menu.open(e, [
            {
              label: "New project…",
              onClick: () => {
                setNewKind("project");
                setCreating(true);
              },
            },
            {
              label: "New area…",
              onClick: () => {
                setNewKind("area");
                setCreating(true);
              },
            },
          ]);
        }}
      >
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
                setSelected(key);
                menu.open(e, projectMenu(p));
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
              <div className="dim tiny">{dirty ? "Unsaved" : ""}</div>
              <div className="grow" />
              {editing === null ? (
                <>
                  <button className="btn" onClick={() => refreshOverview()} disabled={busy}>
                    {busy ? "Refreshing…" : "Refresh summary"}
                  </button>
                  <button
                    className="btn"
                    onClick={() => {
                      setEditing(body);
                      setDirty(false);
                    }}
                  >
                    Edit
                  </button>
                </>
              ) : (
                <>
                  <button
                    className="btn"
                    onClick={() => {
                      setEditing(null);
                      setDirty(false);
                    }}
                  >
                    Cancel
                  </button>
                  <button className="btn primary" onClick={save}>
                    Save
                  </button>
                </>
              )}
            </div>
            <div className={`content ${editing !== null ? "has-editor" : ""}`}>
              {editing !== null ? (
                <NoteEditor
                  value={editing}
                  onChange={(v) => {
                    setEditing(v);
                    setDirty(v !== body);
                  }}
                  vaultPath={vaultPath}
                  onSave={() => void save()}
                />
              ) : (
                <Markdown
                  text={body}
                  vaultPath={vaultPath}
                  onEdit={() => {
                    setEditing(body);
                    setDirty(false);
                  }}
                  extraMenu={
                    selected
                      ? [
                          {
                            label: "Refresh summary",
                            disabled: busy,
                            onClick: () => void refreshOverview(),
                          },
                          {
                            label: "Reveal in vault",
                            onClick: () => {
                              const [kind, slug] = selected.split(":");
                              void api
                                .revealPath(relPath(kind, slug))
                                .catch((e) => onError(errText(e)));
                            },
                          },
                          { kind: "sep" },
                          {
                            label: "Delete",
                            danger: true,
                            onClick: () => {
                              const [kind, slug] = selected.split(":");
                              const p = projects.find(
                                (x) => x.kind === kind && x.slug === slug
                              );
                              setConfirmDelete({
                                kind,
                                slug,
                                name: p?.name ?? slug,
                              });
                            },
                          },
                        ]
                      : undefined
                  }
                />
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
            <p className="dim" style={{ marginTop: 16 }}>
              <button className="btn primary" onClick={() => setCreating(true)}>
                New project / area
              </button>
            </p>
          </div>
        )}
      </div>

      <ContextMenu {...menu.menuProps} />
      <ConfirmDialog
        open={!!confirmDelete}
        title={`Delete ${confirmDelete?.kind ?? ""}?`}
        body={`Remove “${confirmDelete?.name ?? ""}” and drop it from projects.json. Dated history in other notes is left alone.`}
        confirmLabel="Delete"
        danger
        onCancel={() => setConfirmDelete(null)}
        onConfirm={() => void doDelete()}
      />
    </div>
  );
}
