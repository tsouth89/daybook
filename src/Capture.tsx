import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, errText } from "./api";

/**
 * The always-on-top capture overlay. Everything here is in service of one goal:
 * the gap between "I have a thought" and "it is on disk" should be as close to
 * zero as possible. No project picker, no tags, no date field. Dump and dismiss.
 */
export default function Capture() {
  const [text, setText] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    ref.current?.focus();
    const un = listen("capture-focus", () => {
      ref.current?.focus();
      setError(null);
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
      setStatus("Saved");
      setTimeout(() => setStatus(null), 1200);
      await api.hideCapture();
    } catch (e) {
      // Keep the text in the box on failure. Losing a dictation to a disk
      // error would be the single worst thing this app could do.
      setError(errText(e));
    }
  }

  function dismiss() {
    setError(null);
    api.hideCapture();
  }

  async function onPaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    const item = Array.from(e.clipboardData.items).find((i) =>
      i.type.startsWith("image/")
    );
    if (!item) return;
    e.preventDefault();
    const file = item.getAsFile();
    if (!file) return;
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      let bin = "";
      for (const b of buf) bin += String.fromCharCode(b);
      const ext = (file.type.split("/")[1] || "png").replace("jpeg", "jpg");
      const rel = await api.saveAttachment(btoa(bin), ext);
      const el = ref.current;
      const at = el?.selectionStart ?? text.length;
      const md = `\n![](${rel})\n`;
      setText(text.slice(0, at) + md + text.slice(at));
      setStatus("Image attached");
      setTimeout(() => setStatus(null), 1500);
    } catch (err) {
      setError(errText(err));
    }
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      dismiss();
    }
    // Ctrl+Enter rather than Enter: dictation inserts newlines constantly.
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      save();
    }
  }

  return (
    <div className="capture">
      <div className="capture-bar" onMouseDown={() => getCurrentWindow().startDragging()}>
        <span className="capture-title">Daybook</span>
        <span className="capture-hint">Ctrl+Enter save · Esc dismiss</span>
      </div>
      <textarea
        ref={ref}
        className="capture-input"
        value={text}
        placeholder="Dump anything — it'll land in the inbox"
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
            {text.trim() ? `${text.trim().split(/\s+/).length} words` : "Paste images directly"}
          </span>
        )}
        <button className="btn primary" onClick={save} disabled={!text.trim()}>
          Save
        </button>
      </div>
    </div>
  );
}
