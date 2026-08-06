import { createContext, useContext } from "react";

export type AppTab =
  | "inbox"
  | "days"
  | "personal"
  | "projects"
  | "tasks"
  | "ideas"
  | "history"
  | "search"
  | "settings";

/** In-app destination parsed from `[[wiki links]]` or search hits. */
export type NavTarget =
  | { type: "day"; date: string; pane?: "note" | "raw" }
  | { type: "entity"; kind: "project" | "area"; slug: string }
  | { type: "tab"; tab: AppTab };

export type NavigateFn = (target: NavTarget) => void;

const NavContext = createContext<NavigateFn | null>(null);

export function NavProvider({
  navigate,
  children,
}: {
  navigate: NavigateFn;
  children: React.ReactNode;
}) {
  return <NavContext.Provider value={navigate}>{children}</NavContext.Provider>;
}

export function useNavigate(): NavigateFn {
  const nav = useContext(NavContext);
  return nav ?? (() => {});
}

/** Parse vault-relative path or wiki target like `projects/daybook` / `days/2026-08-06`. */
export function pathToNav(path: string): NavTarget | null {
  let p = path.trim().replace(/\\/g, "/").replace(/^\.\//, "").replace(/\.md$/i, "");
  if (!p) return null;

  if (p === "personal" || p === "personal.md") return { type: "tab", tab: "personal" };
  if (p === "tasks" || p === "tasks.md") return { type: "tab", tab: "tasks" };
  if (p === "ideas" || p === "ideas.md") return { type: "tab", tab: "ideas" };

  const day = p.match(/^(?:days\/)?(\d{4}-\d{2}-\d{2})$/);
  if (day) return { type: "day", date: day[1], pane: "note" };

  const raw = p.match(/^raw\/(\d{4}-\d{2}-\d{2})$/);
  if (raw) return { type: "day", date: raw[1], pane: "raw" };

  const proj = p.match(/^projects\/([^/]+)$/);
  if (proj) return { type: "entity", kind: "project", slug: proj[1] };

  const area = p.match(/^areas\/([^/]+)$/);
  if (area) return { type: "entity", kind: "area", slug: area[1] };

  return null;
}

/** Turn Obsidian-style `[[target|label]]` / `[[target]]` into markdown links before marked. */
export function expandWikiLinks(md: string): string {
  return md.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_m, target: string, label?: string) => {
    const t = target.trim();
    const text = (label ?? t).trim();
    const href = `daybook://${encodeURIComponent(t)}`;
    return `[${text}](${href})`;
  });
}
