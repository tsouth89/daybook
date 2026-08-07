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
  /** Route captures automatically once they have sat still. */
  auto_process: boolean;
  /** Seconds a capture must sit untouched first. */
  auto_process_delay_secs: number;
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
  /** active | paused | done — frontmatter, so hand-editable. */
  status: string;
  /** Slug of the entity this nests under; empty at top level. */
  parent: string;
  last_date: string;
  day_count: number;
};

export type ProjectMeta = {
  slug: string;
  name: string;
  kind: string;
  scope: string;
  status: string;
  parent: string;
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

export type Backlink = {
  path: string;
  kind: string;
  line: number;
  text: string;
};

/**
 * One routed entry with its triage properties intact — the queryable half of
 * the vault. `done` is parsed back out of the markdown, not stored.
 */
export type Entry = {
  id: string;
  item_id: string;
  date: string;
  time: string;
  scope: "personal" | "work";
  kind: "project" | "area" | "idea" | "task" | "note";
  /** Owning project/area slug; empty when the entry belongs to nothing. */
  slug: string;
  name: string;
  title: string;
  body: string;
  accomplished: string[];
  decisions: string[];
  open: string[];
  due: string | null;
  done: boolean;
};

export type EntryQuery = {
  scope?: string;
  kind?: string;
  slug?: string;
  date?: string;
  /** Case-insensitive match across an entry's text, including its lists. */
  text?: string;
  /** Inclusive ISO lower bound. */
  since?: string;
  /** Only entries carrying unresolved open loops. */
  open_only?: boolean;
  /** Tasks only: drop the ones already ticked off. */
  undone_only?: boolean;
  limit?: number;
};

export type AskAnswer = {
  answer: string;
  /** The entries the answer was drawn from, so it can be checked. */
  used: Entry[];
};

export type TrashPayload =
  | { kind: "Entry"; record: Entry }
  | { kind: "Entity"; entity_kind: string; slug: string; markdown: string; meta: ProjectMeta | null }
  | { kind: "Inbox"; id: string; contents: string };

export type TrashItem = {
  id: string;
  label: string;
  deleted_at: string;
  payload: TrashPayload;
};

export type RebuildReport = {
  recovered: number;
  kept: number;
  tasks_marked: number;
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
  updateInboxItem: (id: string, text: string) =>
    invoke<void>("update_inbox_item", { id, text }),
  ensureDay: (date?: string) =>
    invoke<string>("ensure_day", { date: date ?? null }),
  todayDate: () => invoke<string>("today_date"),
  listBacklinks: (target: string) =>
    invoke<Backlink[]>("list_backlinks", { target }),
  queryEntries: (query: EntryQuery = {}) =>
    invoke<Entry[]>("query_entries", { query }),
  rebuildEntryIndex: () => invoke<RebuildReport>("rebuild_entry_index"),
  askVault: (question: string) => invoke<AskAnswer>("ask_vault", { question }),
  updateEntry: (entry: Entry) => invoke<void>("update_entry", { entry }),
  createEntry: (entry: Entry) => invoke<Entry>("create_entry", { entry }),
  deleteEntry: (entryId: string) => invoke<void>("delete_entry", { entryId }),
  resolveOpenLoop: (entryId: string, line: string) =>
    invoke<void>("resolve_open_loop", { entryId, line }),
  listTrash: () => invoke<TrashItem[]>("list_trash"),
  restoreTrash: (id: string) => invoke<string>("restore_trash", { id }),
  purgeTrash: (id: string) => invoke<void>("purge_trash", { id }),
  emptyTrash: () => invoke<number>("empty_trash"),
  setEntityParent: (kind: string, slug: string, parent: string) =>
    invoke<void>("set_entity_parent", { kind, slug, parent }),
  setTaskDone: (entryId: string, done: boolean) =>
    invoke<void>("set_task_done", { entryId, done }),
  saveAttachment: (dataBase64: string, ext: string) =>
    invoke<string>("save_attachment", { dataBase64, ext }),
  saveFileAttachment: (dataBase64: string, filename: string) =>
    invoke<string>("save_file_attachment", { dataBase64, filename }),
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
