import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { api, errText, type Entry, type ProjectEntry } from "./api";

type Props = {
  /** An existing entry to correct, or a blank draft to create. */
  entry: Entry | null;
  projects: ProjectEntry[];
  onClose: () => void;
  onSaved: () => void;
  onError: (m: string) => void;
  onNotice?: (m: string) => void;
};

const KINDS = ["task", "note", "idea", "project", "area"] as const;

export function blankEntry(date: string): Entry {
  return {
    id: "",
    item_id: "",
    date,
    time: "",
    scope: "personal",
    kind: "task",
    slug: "",
    name: "",
    title: "",
    body: "",
    accomplished: [],
    decisions: [],
    open: [],
    due: null,
    done: false,
  };
}

/**
 * Correcting triage. Everything here is a property the model guessed at capture
 * time and could not previously be changed from anywhere — not in the app, and
 * not by editing the markdown, since these live in the index.
 */
export default function EntryEditor({
  entry,
  projects,
  onClose,
  onSaved,
  onError,
  onNotice,
}: Props) {
  const [draft, setDraft] = useState<Entry | null>(entry);
  const [busy, setBusy] = useState(false);

  useEffect(() => setDraft(entry), [entry]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  if (!draft) return null;
  const isNew = !draft.id;

  function set<K extends keyof Entry>(k: K, v: Entry[K]) {
    setDraft((d) => (d ? { ...d, [k]: v } : d));
  }

  async function save() {
    if (!draft || busy) return;
    if (!draft.title.trim()) {
      onError("Give it a title.");
      return;
    }
    setBusy(true);
    try {
      // The record carries the project's display name too, so views can show it
      // without a lookup; keep it in step with the slug.
      const name = projects.find((p) => p.slug === draft.slug)?.name ?? "";
      const payload = { ...draft, name: draft.slug ? name : "" };
      if (isNew) {
        await api.createEntry(payload);
        onNotice?.("Added");
      } else {
        await api.updateEntry(payload);
        onNotice?.("Saved");
      }
      onSaved();
      onClose();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    const id = draft?.id;
    if (!id || busy) return;
    setBusy(true);
    try {
      await api.deleteEntry(id);
      onNotice?.("Deleted");
      onSaved();
      onClose();
    } catch (e) {
      onError(errText(e));
    } finally {
      setBusy(false);
    }
  }

  return createPortal(
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div
        className="modal entry-editor"
        onMouseDown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <h3>{isNew ? "New entry" : "Edit entry"}</h3>

        <label>
          <span>Title</span>
          <input
            autoFocus
            value={draft.title}
            onChange={(e) => set("title", e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
            }}
          />
        </label>

        <label>
          <span>Kind</span>
          <select value={draft.kind} onChange={(e) => set("kind", e.target.value as Entry["kind"])}>
            {KINDS.map((k) => (
              <option key={k} value={k}>
                {k}
              </option>
            ))}
          </select>
        </label>

        <label>
          <span>Scope</span>
          <select value={draft.scope} onChange={(e) => set("scope", e.target.value as Entry["scope"])}>
            <option value="personal">personal</option>
            <option value="work">work</option>
          </select>
        </label>

        <label>
          <span>Project</span>
          <select value={draft.slug} onChange={(e) => set("slug", e.target.value)}>
            <option value="">— none —</option>
            {projects.map((p) => (
              <option key={`${p.kind}:${p.slug}`} value={p.slug}>
                {p.name}
              </option>
            ))}
          </select>
        </label>

        <label>
          <span>Date</span>
          <input type="date" value={draft.date} onChange={(e) => set("date", e.target.value)} />
        </label>

        <label>
          <span>Due</span>
          <input
            type="date"
            value={draft.due ?? ""}
            onChange={(e) => set("due", e.target.value || null)}
          />
        </label>

        <label className="tall-label">
          <span>Body</span>
          <textarea
            className="tall"
            value={draft.body}
            onChange={(e) => set("body", e.target.value)}
            spellCheck={false}
          />
        </label>

        {!isNew && draft.kind !== "task" && (
          <p className="dim tiny">
            Editing here changes what views and Ask see. The prose already written onto the
            project or day page is yours to edit there.
          </p>
        )}

        <div className="modal-actions">
          {!isNew && (
            <button className="btn danger" onClick={() => void remove()} disabled={busy}>
              Delete
            </button>
          )}
          <span className="grow" />
          <button className="btn" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button className="btn primary" onClick={() => void save()} disabled={busy}>
            {busy ? "Saving…" : isNew ? "Add" : "Save"}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
