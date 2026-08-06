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
  /** DD/MM/YYYY | MM/DD/YYYY | YYYY-MM-DD */
  date_format: string;
  /** 24h | 12h */
  time_format: string;
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

export type HistoryItem = {
  id: string;
  date: string;
  time: string;
  preview: string;
  chars: number;
  has_day_note: boolean;
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
  attachmentDataUrl: (rel: string) => invoke<string>("attachment_data_url", { rel }),
  hideCapture: () => invoke<void>("hide_capture"),
  showCapture: () => invoke<void>("show_capture"),

  listDays: () => invoke<DayEntry[]>("list_days"),
  readDay: (date: string) => invoke<DayContent>("read_day", { date }),
  writeRaw: (date: string, content: string) =>
    invoke<void>("write_raw", { date, content }),
  writeNote: (date: string, content: string) =>
    invoke<void>("write_note", { date, content }),

  listProjects: () => invoke<ProjectEntry[]>("list_projects"),
  readProject: (slug: string) => invoke<string>("read_project", { slug }),
  readEntity: (kind: string, slug: string) =>
    invoke<string>("read_entity", { kind, slug }),
  writeEntity: (kind: string, slug: string, content: string) =>
    invoke<void>("write_entity", { kind, slug, content }),
  createEntity: (kind: string, name: string, scope: string) =>
    invoke<ProjectMeta>("create_entity", { kind, name, scope }),
  deleteEntity: (kind: string, slug: string) =>
    invoke<void>("delete_entity", { kind, slug }),
  refreshEntityOverview: (kind: string, slug: string) =>
    invoke<string>("refresh_entity_overview", { kind, slug }),
  refreshPersonalOverview: () => invoke<string>("refresh_personal_overview"),

  search: (query: string) => invoke<SearchHit[]>("search", { query }),

  getProjectsConfig: () => invoke<ProjectMeta[]>("get_projects_config"),
  saveProjectsConfig: (projects: ProjectMeta[]) =>
    invoke<void>("save_projects_config", { projects }),

  getGlossary: () => invoke<string>("get_glossary"),
  saveGlossary: (text: string) => invoke<void>("save_glossary", { text }),
  getProfile: () => invoke<string>("get_profile"),
  saveProfile: (text: string) => invoke<void>("save_profile", { text }),

  revealVault: () => invoke<void>("reveal_vault"),
  revealPath: (rel: string) => invoke<void>("reveal_path", { rel }),
  readTasks: () => invoke<string>("read_tasks"),
  readIdeas: () => invoke<string>("read_ideas"),
  readPersonal: () => invoke<string>("read_personal"),
  writePersonal: (content: string) => invoke<void>("write_personal", { content }),
  writeIdeas: (content: string) => invoke<void>("write_ideas", { content }),
  writeTasks: (content: string) => invoke<void>("write_tasks", { content }),
  listHistory: () => invoke<HistoryItem[]>("list_history"),
  readHistoryItem: (date: string, id: string) =>
    invoke<string>("read_history_item", { date, id }),
  toggleTaskLine: (line: number) => invoke<string>("toggle_task_line", { line }),
  processInbox: (date?: string) =>
    invoke<InboxProcessResult>("process_inbox", { date: date ?? null, id: null }),
  processInboxItem: (id: string) =>
    invoke<InboxProcessResult>("process_inbox_item", { id }),
  processDay: (date: string) => invoke<InboxProcessResult>("process_day", { date }),
};

/** Errors from `invoke` arrive as plain strings, not Error instances. */
export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
