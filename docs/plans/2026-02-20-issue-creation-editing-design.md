# Issue Creation & Editing

## Overview

Add in-app issue creation and editing via a form view pushed onto the view stack. Context-sensitive creation (inherits epic when creating from an epic view). Default labels from config. Description editing via `$EDITOR`.

## Architecture

### Data Flow

1. TicketPanel emits `CreateRequested` / `EditRequested(IssueSummary)` on 'c' / 'e' keys
2. Parent view catches the event, supplies context (`project`, optional `epic`)
3. Parent pushes `IssueEditorView` onto the view stack
4. On save, editor calls Jira API, then pops. Parent auto-refreshes via `View::on_resume()`

### New Jira API Methods

- `create_issue(project, summary, issue_type, description, labels, epic)` → POST `/rest/api/2/issue`
- `update_issue(key, summary, description, issue_type, labels, epic)` → PUT `/rest/api/2/issue/{key}`
- `get_project_statuses(project)` → GET `/rest/api/2/project/{project}/statuses` (returns issue types + statuses per type)

Create-then-transition: after creation, if the user selected a non-default status, call `update_issue_status(key, status_id)` to transition.

### Config Addition

Top-level `default_labels: Vec<String>` in config:

```yaml
default_labels:
  - team-foo
  - needs-triage
```

## Editor View UX

### Form Layout

```
┌─ Create Issue ──────────────────────────┐
│                                         │
│  Title:       [_________________________]│
│  Type:        Task ▾                    │
│  Status:      To Do ▾                  │
│  Epic:        PROJ-42 ▾                │
│  Labels:      [team-foo, needs-triage__] │
│  Description: (press Enter to edit)     │
│                                         │
│           Ctrl+S Submit   Esc Cancel    │
└─────────────────────────────────────────┘
```

### Field Types

- **Title** — TextInput, single-line. Focused by default.
- **Type** — FieldPicker. Options from `get_project_statuses`.
- **Status** — FieldPicker. Options filtered by selected issue type.
- **Epic** — FieldPicker. Options from `get_epics(project)`. Optional (can be "None"). Pre-filled from context.
- **Labels** — TextInput, comma-separated. Pre-filled with `default_labels` on create.
- **Description** — Shows preview. Enter launches `$EDITOR` with temp file.

### Navigation

- Tab / Shift+Tab, j/k, Up/Down — move between fields (j/k/arrows only when not in TextInput)
- Enter on picker — open overlay
- Enter on Description — launch `$EDITOR`
- Ctrl+S — submit
- Esc — cancel (with confirmation if dirty)

### Data Loading

- Create: fetch project statuses + epics in parallel
- Edit: fetch full issue + project statuses + epics in parallel, pre-fill fields

### Submit

- Create: validate title required → `create_issue(...)` → if non-default status, `update_issue_status(...)` → Pop
- Edit: `update_issue(...)` → if status changed, `update_issue_status(...)` → Pop

## Component Structure

### New Files

- `src/ui/views/issue_editor.rs` — IssueEditorView
- `src/ui/components/field_picker.rs` — generic picker overlay (reusable for type/status/epic)

### Modified Files

- `src/ui/components/ticket_panel.rs` — add CreateRequested/EditRequested events, 'c'/'e' keys
- `src/ui/view.rs` — add `fn on_resume(&mut self) {}` to View trait
- `src/app.rs` — call `on_resume()` on Pop, pass default_labels when constructing editors
- `src/config.rs` — add `default_labels: Vec<String>`
- `src/jira/client.rs` — add `create_issue()`, `update_issue()`, `get_project_statuses()`
- `src/jira/types.rs` — add `IssueTypeWithStatuses` type
- `src/ui/views/issue_list.rs` — handle create/edit events, implement on_resume
- `src/ui/views/epic_detail.rs` — handle create/edit events (with epic context), implement on_resume
- `src/ui/views/epic_list.rs` — handle create/edit events, implement on_resume
