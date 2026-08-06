import { useEffect, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";
import Markdown from "./Markdown";
import type { MenuItem } from "./ContextMenu";

export type EditorMode = "live" | "source";

type Props = {
  /** Markdown being edited. */
  value: string;
  onChange: (next: string) => void;
  vaultPath?: string;
  /** Prefer live (split) or source-only when editing starts. */
  initialMode?: EditorMode;
  onSave?: () => void;
  extraMenu?: MenuItem[];
};

const editorTheme = EditorView.theme({
  "&": { height: "100%", fontSize: "13px" },
  ".cm-scroller": { overflow: "auto", fontFamily: '"Cascadia Mono", Consolas, monospace' },
  ".cm-content": { padding: "16px 18px", caretColor: "#dfe3ea" },
  ".cm-gutters": { background: "transparent", border: "none" },
});

/**
 * Obsidian-ish editing: Live (source + rendered preview) or Source-only.
 * Ctrl/Cmd+S triggers onSave when provided.
 */
export default function NoteEditor({
  value,
  onChange,
  vaultPath,
  initialMode = "live",
  onSave,
  extraMenu,
}: Props) {
  const [mode, setMode] = useState<EditorMode>(initialMode);

  useEffect(() => {
    if (!onSave) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        onSave();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onSave]);

  return (
    <div className={`note-editor mode-${mode}`}>
      <div className="note-editor-bar">
        <div className="tabs">
          <button
            type="button"
            className={`tab ${mode === "live" ? "active" : ""}`}
            onClick={() => setMode("live")}
          >
            Live
          </button>
          <button
            type="button"
            className={`tab ${mode === "source" ? "active" : ""}`}
            onClick={() => setMode("source")}
          >
            Source
          </button>
        </div>
        <span className="dim tiny">{onSave ? "Ctrl+S to save" : ""}</span>
      </div>
      <div className="note-editor-body">
        <div className="note-source">
          <CodeMirror
            value={value}
            height="100%"
            theme={oneDark}
            extensions={[markdown(), editorTheme, EditorView.lineWrapping]}
            onChange={onChange}
            basicSetup={{
              lineNumbers: true,
              foldGutter: true,
              highlightActiveLine: true,
            }}
          />
        </div>
        {mode === "live" && (
          <div className="note-preview">
            <Markdown text={value} vaultPath={vaultPath} extraMenu={extraMenu} />
          </div>
        )}
      </div>
    </div>
  );
}
