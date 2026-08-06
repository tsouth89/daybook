import { invoke } from "@tauri-apps/api/core";

export type Settings = {
  vault_path: string;
  provider: "deepseek" | "openai" | "anthropic";
  model: string;
  deepseek_api_key: string;
  openai_api_key: string;
  anthropic_api_key: string;
  /** @deprecated migrated into anthropic_api_key */
  api_key: string;
  effort: "low" | "medium" | "high" | "xhigh" | "max";
  capture_hotkey: string;
  context_days: number;
};

export type DayEntry = {
  date: string;
  has_raw: boolean;
  has_note: boolean;
  raw_chars: number;
  preview: string;
};

export type DayContent = { date: string; raw: string; note: string };

export type ProjectEntry = {
  slug: string;
  name: string;
  kind: string;
  scope: string;
  last_date: string;
  day_count: number;
};

export type ProjectMeta = {
  slug: string;
  name: string;
  kind: string;
  scope: string;
  aliases: string[];
  description: string;
};

export type InboxItem = {
  id: string;
  date: string;
  time: string;
  text: string;
  chars: number;
};

export type SearchHit = {
  path: string;
  kind: string;
  date: string;
  line: number;
  text: string;
};

export type ItemProcessResult = {
  id: string;
  date: string;
  entry_count: number;
  destinations: string[];
  new_entities: string[];
  summary: string[];
};

export type InboxProcessResult = {
  processed: ItemProcessResult[];
  errors: string[];
};

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),

  appendEntry: (text: string) => invoke<string>("append_entry", { text }),
  listInbox: () => invoke<InboxItem[]>("list_inbox"),
  deleteInboxItem: (id: string) => invoke<void>("delete_inbox_item", { id }),
  saveAttachment: (dataBase64: string, ext: string) =>
    invoke<string>("save_attachment", { dataBase64, ext }),
  hideCapture: () => invoke<void>("hide_capture"),

  listDays: () => invoke<DayEntry[]>("list_days"),
  readDay: (date: string) => invoke<DayContent>("read_day", { date }),
  writeRaw: (date: string, content: string) =>
    invoke<void>("write_raw", { date, content }),

  listProjects: () => invoke<ProjectEntry[]>("list_projects"),
  readProject: (slug: string) => invoke<string>("read_project", { slug }),
  readEntity: (kind: string, slug: string) =>
    invoke<string>("read_entity", { kind, slug }),

  search: (query: string) => invoke<SearchHit[]>("search", { query }),

  getProjectsConfig: () => invoke<ProjectMeta[]>("get_projects_config"),
  saveProjectsConfig: (projects: ProjectMeta[]) =>
    invoke<void>("save_projects_config", { projects }),

  getGlossary: () => invoke<string>("get_glossary"),
  saveGlossary: (text: string) => invoke<void>("save_glossary", { text }),
  getProfile: () => invoke<string>("get_profile"),
  saveProfile: (text: string) => invoke<void>("save_profile", { text }),

  revealVault: () => invoke<void>("reveal_vault"),
  processInbox: (date?: string) =>
    invoke<InboxProcessResult>("process_inbox", { date: date ?? null }),
  processDay: (date: string) => invoke<InboxProcessResult>("process_day", { date }),
};

/** Errors from `invoke` arrive as plain strings, not Error instances. */
export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
