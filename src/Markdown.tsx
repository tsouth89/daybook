import { useEffect, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { api } from "./api";

type Props = {
  text: string;
  /** Vault root — kept for API compatibility; images load via Tauri command. */
  vaultPath?: string;
};

function collectAttachmentSrcs(html: string): string[] {
  const out: string[] = [];
  const re = /<img[^>]*?\ssrc="([^"]+)"/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(html))) {
    const src = m[1];
    if (src.includes("attachments/") && !src.startsWith("data:")) {
      const rel = src.replace(/^\.\//, "").replace(/^\//, "");
      if (!out.includes(rel)) out.push(rel);
    }
  }
  return out;
}

async function resolveVaultImages(html: string): Promise<string> {
  const refs = collectAttachmentSrcs(html);
  if (!refs.length) return html;
  let out = html;
  for (const rel of refs) {
    try {
      const dataUrl = await api.attachmentDataUrl(rel);
      // Replace every src that points at this relative path.
      out = out.replace(
        new RegExp(`src="${rel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`, "g"),
        `src="${dataUrl}"`
      );
      out = out.replace(
        new RegExp(`src="\\./${rel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`, "g"),
        `src="${dataUrl}"`
      );
    } catch {
      /* leave broken src — better than crashing the note view */
    }
  }
  return out;
}

/**
 * Notes are local and self-generated, but they contain whatever the dictation
 * picked up, so sanitise before injecting rather than trusting the source.
 * Vault images are loaded as data URLs so they work without asset-protocol scope fights.
 */
export default function Markdown({ text }: Props) {
  const [html, setHtml] = useState("");

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const raw = marked.parse(text || "", { async: false }) as string;
      const withImages = await resolveVaultImages(raw);
      if (!cancelled) setHtml(DOMPurify.sanitize(withImages));
    })();
    return () => {
      cancelled = true;
    };
  }, [text]);

  if (!text.trim()) return <p className="dim">Nothing here yet.</p>;
  if (!html) return <p className="dim">…</p>;
  return <div className="md" dangerouslySetInnerHTML={{ __html: html }} />;
}
