import { useEffect, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { api } from "./api";
import { ContextMenu, copyText, useContextMenu, type MenuItem } from "./ContextMenu";

type Props = {
  text: string;
  /** Vault root — kept for API compatibility; images load via Tauri command. */
  vaultPath?: string;
  /** Extra menu items after Copy (e.g. Edit). */
  extraMenu?: MenuItem[];
  onEdit?: () => void;
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
      out = out.replace(
        new RegExp(`src="${rel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`, "g"),
        `src="${dataUrl}"`
      );
      out = out.replace(
        new RegExp(`src="\\./${rel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"`, "g"),
        `src="${dataUrl}"`
      );
    } catch {
      /* leave broken src */
    }
  }
  return out;
}

/**
 * Notes are local and self-generated, but they contain whatever the dictation
 * picked up, so sanitise before injecting rather than trusting the source.
 * Vault images are loaded as data URLs so they work without asset-protocol scope fights.
 */
export default function Markdown({ text, extraMenu, onEdit }: Props) {
  const [html, setHtml] = useState("");
  const menu = useContextMenu();

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

  return (
    <>
      <div
        className="md"
        dangerouslySetInnerHTML={{ __html: html }}
        onContextMenu={(e) => {
          const sel = window.getSelection()?.toString() ?? "";
          const items: MenuItem[] = [
            {
              label: "Copy selection",
              disabled: !sel.trim(),
              onClick: () => void copyText(sel),
            },
            {
              label: "Copy all",
              onClick: () => void copyText(text),
            },
          ];
          if (onEdit) {
            items.push({ kind: "sep" }, { label: "Edit", onClick: onEdit });
          }
          if (extraMenu?.length) {
            items.push({ kind: "sep" }, ...extraMenu);
          }
          menu.open(e, items);
        }}
      />
      <ContextMenu {...menu.menuProps} />
    </>
  );
}
