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
projects/<slug>.md         things with an end state
areas/<slug>.md            ongoing responsibilities (health, finances, house)
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
- **Inbox**: review pending captures, then Process. One dump that mentions a bug fix,
  a dentist appointment, and a side idea becomes three routed entries.
- **Days / Projects**: read what was filed. Projects and areas share one list; filter
  by kind or scope.
- **Tune**: glossary first (dictation mangles proper nouns), then aliases, then the
  profile. Those three beat any model upgrade.

## Cost

Default model is DeepSeek V4 Flash (~$0.14/$0.28 per Mtok). A normal day of triage is
fractions of a cent. OpenAI Luna/Terra and Anthropic Claude are available in Settings if
Flash misroutes or you want a stronger pass later.

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
