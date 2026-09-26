# opx

A terminal runbook/checklist CLI that treats Markdown as the source of truth. Checked state is saved directly in Markdown task markers (`[ ]` / `[x]`). No separate database or state file is created.

## Installation

Install Rust stable, then run this from the repository root:

```sh
cargo install --path .
```

## Markdown example

````md
# Deploy

## Build

- [ ] Build the Docker image

```sh
docker build -t app .
```

- [ ] Verify the image
````

A fenced code block immediately following a task (blank lines are allowed) is associated with that task as its command. Commands are never executed automatically.

## CLI

Opening a Markdown file is the default operation and starts the TUI:

```sh
opx runbook.md
```

Use the `cli` namespace for non-interactive operations:

```sh
opx cli status runbook.md
opx cli status runbook.md --json
opx cli next runbook.md --json
opx cli check runbook.md 0
opx cli uncheck runbook.md 0
opx cli copy runbook.md 0
```

Task indexes start at 0. Normal output goes to stdout, errors and warnings go to stderr, and `next` exits non-zero when there are no unchecked tasks.

Example JSON:

```json
{
  "index": 1,
  "section": "Database",
  "title": "Run the migration",
  "checked": false,
  "language": "sh",
  "command": "bundle exec rails db:migrate"
}
```

`status --json` returns `total`, `completed`, `progress`, and `tasks`. For nested headings, `section` contains the nearest heading and `section_path` contains the full hierarchy from the top-level heading.

Normal text between a task and the next task or heading is associated with that task as `details`. It is shown in the TUI detail panel and included in JSON. A fenced code block immediately following a task is treated as its command rather than as `details`.

## TUI key bindings

| Key | Action |
| --- | --- |
| `j` / `Down` | Move to the next task |
| `k` / `Up` | Move to the previous task |
| `Space` / `x` | Toggle checked / unchecked and save immediately |
| `y` | Copy the associated command to the clipboard |
| `r` | Ask for confirmation before running the associated command |
| `Enter` | Open or close the right-side detail panel |
| `n` | Move to the next unchecked task |
| `q` | Quit |

The TUI displays the completed count and a progress bar at the top. The selected task uses a bright blue-purple color, completed tasks use a muted color with a strikethrough on the title, and unchecked tasks use gray. Pressing `y` shows the copy result at the bottom. The detail panel shows the task status, heading hierarchy, language, command, and task-adjacent details.

Pressing `r` never runs a command immediately. The TUI asks for confirmation; only `y` executes the selected command, while `n` or `Esc` cancels it. The TUI temporarily leaves the alternate screen so command output is visible, then returns to the runbook. Copying a command only writes it to the clipboard.
