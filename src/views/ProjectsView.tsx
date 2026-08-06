import { useEffect, useState } from "react";
import { api, errText, type ProjectEntry } from "../api";
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
  const [filter, setFilter] = useState<"all" | "project" | "area" | "personal" | "work">(
    "all"
  );

  useEffect(() => {
    api.listProjects().then(setProjects).catch((e) => onError(errText(e)));
  }, [onError]);

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
    const [kind, slug] = selected.split(":");
    api.readEntity(kind, slug).then(setBody).catch((e) => onError(errText(e)));
  }, [selected, onError]);

  if (!projects.length) {
    return (
      <div className="empty">
        <h2>No projects or areas yet</h2>
        <p className="dim">
          They appear once inbox items are processed. Projects have an end state; areas
          (health, finances, the house) do not.
        </p>
      </div>
    );
  }

  return (
    <div className="split">
      <div className="list">
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
        {visible.map((p) => {
          const key = `${p.kind}:${p.slug}`;
          return (
            <button
              key={key}
              className={`listitem ${selected === key ? "active" : ""}`}
              onClick={() => setSelected(key)}
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
        {visible.length === 0 && (
          <p className="dim tiny" style={{ padding: "12px" }}>
            Nothing in this filter.
          </p>
        )}
      </div>
      <div className="detail">
        <div className="content">
          <Markdown text={body} vaultPath={vaultPath} />
        </div>
      </div>
    </div>
  );
}
