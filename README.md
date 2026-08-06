# Daybook

A voice-first personal journal. Dump thoughts via hotkey; AI splits them up and
files each piece where it belongs — a project, an area of life, an idea, a task,
or just today's note.

## The one design rule

**The AI never edits your raw dump.** Captures land in `inbox/`, and after triage
the verbatim text is archived append-only in `raw/`. Everything else in the vault
is a build artifact generated from that.

That is what makes the whole thing improvable: when routing gets better, you can
delete `days/`, `projects/`, and `areas/` and rebuild. A bad model run can never
destroy the source.

## Vault layout

```
inbox/                     ingress. Anything can drop a file here.
raw/YYYY-MM-DD.md          append-only archive of triaged items. Source of truth.
days/YYYY-MM-DD.md         generated view over the day's entries
projects/<slug>.md         project home (overview + dated log)
areas/<slug>.md            ongoing responsibilities (health, finances, house)
personal.md                rollup of personal-scoped entries over time
ideas.md                   maybe-someday, dated
tasks.md                   open checkboxes, dated
attachments/               pasted images
config/projects.json       known projects/areas + aliases
config/glossary.txt        jargon list, repairs dictation errors
config/profile.md          durable facts about you, fed to every pass
```

**Scope and destination are independent.** Every entry gets a scope (`personal` or
`work`) and a destination (`project`, `area`, `idea`, `task`, or day-only `note`).
A personal project is just scope=personal + kind=project — no taxonomy collapse.
Personal-scoped entries also appear in `personal.md` so you can read life stuff in one place.

Plain Markdown with YAML frontmatter, so the vault is also a valid Obsidian vault.
Point Obsidian at the same folder for graph view, backlinks, or hand-written notes
in the right place when you want that instead of dumping into the inbox.

## Flow

1. Capture writes a discrete item file to `inbox/`
2. Triage reads pending items, splits each into entries, assigns scope + destination
3. Entries append to their destinations; the verbatim item appends to `raw/`; the
   inbox file is deleted only after both succeed
4. The day note is updated as a view over those entries

A failed triage leaves the item sitting in the inbox.

## Using it

- **Capture**: `Ctrl+Shift+Space` opens the overlay. Dictate, `Ctrl+Enter` to save,
  `Esc` to dismiss. Images paste straight in. Text stays in the box if a save fails.
- **Today**: where the app opens. Today's note, what is still waiting in the inbox for
  today, and one button to file it. The sidebar badge is today's pending count.
- **Inbox**: review pending captures, then Process. One dump that mentions a bug fix,
  a dentist appointment, and a side idea becomes three routed entries. Captures are
  editable before processing — fix a mangled word instead of discarding and redictating.
- **Days / Projects**: read what was filed. Projects and areas share one list; filter
  by kind or scope. Each page lists what links to it under "Linked from".
- **Tune**: glossary first (dictation mangles proper nouns), then aliases, then the
  profile. Those three beat any model upgrade.

### Keyboard

In-app shortcuts, ignored while you are typing in an editor or input:

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+T` | Today |
| `Ctrl+Shift+I` | Inbox |
| `Ctrl+Shift+D` | Days |
| `Ctrl+Shift+J` | Projects |
| `Ctrl+Shift+F` | Search |
| `Ctrl+Shift+E` | New entry |
| `Ctrl+Shift+P` | Process the current view's pending items |
| `Ctrl+S` | Save the open editor |

### Editing by hand

Day notes, project pages, areas, and `personal.md` are all safe to edit directly —
in the app or in Obsidian. Processing a new capture splices that item's section into
the day note rather than regenerating the file, so headings, prose, and bullets you
wrote yourself survive. The same holds for the `## Overview` block on a project, area,
or personal page: it is only rewritten when you ask for **Refresh summary**.

`raw/` is the exception. It is append-only and the app warns before letting you edit it.

## Cost

Default model is GPT-5.6 Luna (~$0.20/$1.20 per Mtok). A normal month of triage is well under
a dollar. Terra and Anthropic Claude are available if you need more power; DeepSeek remains an
option if you ever want the absolute cheapest pass.

## Development

```sh
pnpm install
pnpm tauri dev      # run
pnpm tauri build    # bundle
```

Rust backend under `src-tauri/src`:

- `vault.rs` — file IO and the vault layout
- `ai.rs` — triage call, prompt, and Markdown renderers
- `config.rs` — settings and API key resolution
- `lib.rs` — Tauri commands, global hotkey, window wiring
