import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errText } from "./api";

function toBase64(buf: Uint8Array): string {
  // Chunked because String.fromCharCode(...spread) blows the stack on big files.
  let bin = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < buf.length; i += CHUNK) {
    bin += String.fromCharCode(...buf.subarray(i, i + CHUNK));
  }
  return btoa(bin);
}

/**
 * Store a copy of a dropped file and reference it in the capture text. Images
 * embed; anything else becomes a plain link, since `![]()` around a PDF just
 * renders broken.
 */
async function attachFile(
  file: File,
  text: string,
  cursor: number
): Promise<{ text: string; cursor: number }> {
  const buf = new Uint8Array(await file.arrayBuffer());
  const b64 = toBase64(buf);
  const isImage = file.type.startsWith("image/");

  let rel: string;
  if (isImage && !file.name) {
    const ext = (file.type.split("/")[1] || "png").replace("jpeg", "jpg");
    rel = await api.saveAttachment(b64, ext);
  } else {
    rel = await api.saveFileAttachment(b64, file.name || "file");
  }

  const label = file.name || rel.split("/").pop() || "file";
  const md = isImage ? `\n![](${rel})\n` : `\n[${label}](${rel})\n`;
  const next = text.slice(0, cursor) + md + text.slice(cursor);
  return { text: next, cursor: cursor + md.length };
}

/**
 * The always-on-top capture overlay. Everything here is in service of one goal:
 * the gap between "I have a thought" and "it is on disk" should be as close to
 * zero as possible. No project picker, no tags, no date field. Dump and dismiss.
 *
 * Spokenly (or any dictation tool) types into the focused textarea like a keyboard.
 */
export default function Capture() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    ref.current?.focus();
    const un = listen("capture-focus", () => {
      setError(null);
      ref.current?.focus();
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  async function save() {
    if (!text.trim()) {
      dismiss();
      return;
    }
    try {
      await api.appendEntry(text);
      setText("");
      setError(null);
      setStatus("Saved to inbox");
      setTimeout(() => setStatus(null), 1200);
      await api.hideCapture();
    } catch (e) {
      setError(errText(e));
    }
  }

  function dismiss() {
    setError(null);
    api.hideCapture();
  }

  async function ingest(file: File) {
    try {
      setStatus(`Storing ${file.name || "file"}…`);
      const at = ref.current?.selectionStart ?? text.length;
      const next = await attachFile(file, text, at);
      setText(next.text);
      setStatus(file.type.startsWith("image/") ? "Image attached" : `Attached ${file.name}`);
      setTimeout(() => setStatus(null), 1500);
      ref.current?.focus();
    } catch (err) {
      setStatus(null);
      setError(errText(err));
    }
  }

  async function onPaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    const item = Array.from(e.clipboardData.items).find(
      (i) => i.kind === "file"
    );
    if (!item) return;
    e.preventDefault();
    const file = item.getAsFile();
    if (file) await ingest(file);
  }

  function onDragOver(e: React.DragEvent) {
    e.preventDefault();
    setDragOver(true);
  }

  function onDragLeave(e: React.DragEvent) {
    e.preventDefault();
    setDragOver(false);
  }

  async function onDrop(e: React.DragEvent) {
    e.preventDefault();
    setDragOver(false);
    for (const file of Array.from(e.dataTransfer.files)) {
      await ingest(file);
    }
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      dismiss();
    }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      save();
    }
  }

  return (
    <div
      className={`capture ${dragOver ? "drag-over" : ""}`}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      <div className="capture-bar" onMouseDown={() => getCurrentWindow().startDragging()}>
        <span className="capture-title">Daybook</span>
        <span className="capture-hint">Ctrl+Enter save · Esc dismiss</span>
      </div>
      <textarea
        ref={ref}
        className="capture-input"
        value={text}
        placeholder="Dictate, type, or drop anything — lands in inbox"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
        spellCheck={false}
      />
      <div className="capture-foot">
        {error ? (
          <span className="bad">{error}</span>
        ) : status ? (
          <span className="good">{status}</span>
        ) : (
          <span className="dim">
            {text.trim()
              ? `${text.trim().split(/\s+/).length} words`
              : "Paste or drag files · markdown OK"}
          </span>
        )}
        <button className="btn primary" onClick={save} disabled={!text.trim()}>
          Save
        </button>
      </div>
    </div>
  );
}
