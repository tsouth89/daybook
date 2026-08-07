import {
  autocompletion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import type { Extension } from "@codemirror/state";
import { api } from "./api";

export type LinkTarget = { label: string; target: string; detail: string };

/**
 * Everything worth linking to. Cached for the life of an editor session —
 * typing `[[` should never wait on disk.
 */
async function loadTargets(): Promise<LinkTarget[]> {
  const out: LinkTarget[] = [];
  try {
    for (const p of await api.listProjects()) {
      out.push({
        label: p.name,
        target: `${p.kind === "area" ? "areas" : "projects"}/${p.slug}`,
        detail: p.kind,
      });
    }
  } catch {
    /* a vault with no projects yet is not an error */
  }
  try {
    for (const d of await api.listDays()) {
      out.push({ label: d.date, target: `days/${d.date}`, detail: "day" });
    }
  } catch {
    /* same */
  }
  for (const page of ["personal", "tasks", "ideas"]) {
    out.push({ label: page, target: page, detail: "page" });
  }
  return out;
}

/**
 * Obsidian's core gesture: type `[[`, get the thing you meant. Without it you
 * have to already know the exact slug, which defeats the point of links.
 */
export function wikilinkCompletion(): Extension {
  let cache: LinkTarget[] | null = null;
  let inflight: Promise<LinkTarget[]> | null = null;

  const targets = () => {
    if (cache) return Promise.resolve(cache);
    inflight ??= loadTargets().then((t) => {
      cache = t;
      inflight = null;
      return t;
    });
    return inflight;
  };

  return autocompletion({
    override: [
      async (ctx: CompletionContext): Promise<CompletionResult | null> => {
        // Match an unclosed `[[` and whatever has been typed since.
        const before = ctx.matchBefore(/\[\[[^\]\n]*/);
        if (!before) return null;
        if (before.from === before.to && !ctx.explicit) return null;

        const typed = before.text.slice(2).toLowerCase();
        const all = await targets();
        const options = all
          .filter(
            (t) =>
              !typed ||
              t.label.toLowerCase().includes(typed) ||
              t.target.toLowerCase().includes(typed)
          )
          .slice(0, 50)
          .map((t) => ({
            label: t.target,
            displayLabel: t.label,
            detail: t.detail,
            // Close the brackets so the link is valid the moment it is picked.
            apply: `${t.target}]]`,
          }));

        if (!options.length) return null;
        return { from: before.from + 2, options, filter: false };
      },
    ],
  });
}
