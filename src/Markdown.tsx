import { useEffect, useRef, useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { api } from "./api";
import { ContextMenu, copyText, useContextMenu, type MenuItem } from "./ContextMenu";
import { expandWikiLinks, pathToNav, useNavigate } from "./nav";

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
 * `[[wiki links]]` become in-app navigable anchors.
 */
export default function Markdown({ text, extraMenu, onEdit }: Props) {
  const [html, setHtml] = useState("");
  const menu = useContextMenu();
  const navigate = useNavigate();
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const expanded = expandWikiLinks(text || "");
      const raw = marked.parse(expanded, { async: false }) as string;
      const withImages = await resolveVaultImages(raw);
      if (!cancelled) {
        setHtml(
          DOMPurify.sanitize(withImages, {
            ALLOWED_URI_REGEXP:
              /^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp|daybook):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
          })
        );
      }
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
        ref={rootRef}
        className="md"
        dangerouslySetInnerHTML={{ __html: html }}
        onClick={(e) => {
          const a = (e.target as HTMLElement).closest("a");
          if (!a || !rootRef.current?.contains(a)) return;
          const href = a.getAttribute("href") || "";
          if (href.startsWith("daybook://")) {
            e.preventDefault();
            const target = decodeURIComponent(href.slice("daybook://".length));
            const nav = pathToNav(target);
            if (nav) navigate(nav);
          }
        }}
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
