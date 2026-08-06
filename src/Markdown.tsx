import { useMemo } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";

/**
 * Notes are local and self-generated, but they contain whatever the dictation
 * picked up, so sanitise before injecting rather than trusting the source.
 */
export default function Markdown({ text }: { text: string }) {
  const html = useMemo(() => {
    const raw = marked.parse(text || "", { async: false }) as string;
    return DOMPurify.sanitize(raw);
  }, [text]);

  if (!text.trim()) return <p className="dim">Nothing here yet.</p>;
  return <div className="md" dangerouslySetInnerHTML={{ __html: html }} />;
}
