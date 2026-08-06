import { useMemo } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { marked } from "marked";
import DOMPurify from "dompurify";

type Props = {
  text: string;
  /** Vault root — required for `attachments/…` images to render. */
  vaultPath?: string;
};

function rewriteVaultImages(html: string, vaultPath: string): string {
  const root = vaultPath.replace(/\\/g, "/").replace(/\/$/, "");
  return html.replace(/<img([^>]*?)src="([^"]+)"([^>]*)>/g, (_m, pre, src, post) => {
    if (
      src.startsWith("http://") ||
      src.startsWith("https://") ||
      src.startsWith("data:") ||
      src.startsWith("asset://")
    ) {
      return `<img${pre}src="${src}"${post}>`;
    }
    const rel = src.replace(/^\.\//, "").replace(/^\//, "");
    const abs = `${root}/${rel}`;
    const url = convertFileSrc(abs);
    return `<img${pre}src="${url}"${post}>`;
  });
}

/**
 * Notes are local and self-generated, but they contain whatever the dictation
 * picked up, so sanitise before injecting rather than trusting the source.
 */
export default function Markdown({ text, vaultPath }: Props) {
  const html = useMemo(() => {
    const raw = marked.parse(text || "", { async: false }) as string;
    const safe = DOMPurify.sanitize(raw);
    if (!vaultPath?.trim()) return safe;
    return rewriteVaultImages(safe, vaultPath);
  }, [text, vaultPath]);

  if (!text.trim()) return <p className="dim">Nothing here yet.</p>;
  return <div className="md" dangerouslySetInnerHTML={{ __html: html }} />;
}
