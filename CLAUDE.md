# j9s Development Notes

## Code Style

- Rust formatting: 100 columns, 2-space indentation (see `rustfmt.toml`)
- Run `cargo fmt` before committing

## Project Structure

- `src/app.rs` - Main app state and event loop
- `src/event.rs` - Event types and channel setup
- `src/config.rs` - XDG-compliant config loading
- `src/ui/` - All UI code (see below)
- `src/jira/` - Gouqi wrapper and types
- `src/db/` - SQLite caching (not yet integrated)

## UI Architecture

```
src/ui/
├── mod.rs           # Main draw function
├── view.rs          # View trait and ViewAction enum
├── components/      # Stateful components with co-located rendering
│   ├── input.rs         # TextInput - base input component
│   ├── search_input.rs  # SearchInput - wraps TextInput, renders overlay
│   └── command_input.rs # CommandInput - wraps TextInput + autocomplete
├── views/           # View structs with co-located rendering
│   ├── issue_list.rs    # IssueListView
│   ├── board_list.rs    # BoardListView
│   └── issue_detail.rs  # IssueDetailView
└── renderfns/       # Purely stateless render functions
    ├── header.rs
    └── footer.rs
```

**Delegation chain:** App → View → Component

- **App** owns command mode (`:` prefix), delegates to current view
- **Views** own their modes (e.g. search with `/`), implement `View` trait
- **Components** are reusable input handlers (TextInput, SearchInput, CommandInput)
- **renderfns** are stateless - just take data and render, no state

When adding a new view: create in `ui/views/`, implement `View` trait, co-locate rendering.
When adding a new component: create in `ui/components/`, co-locate rendering with the component.

## Navigation

Uses k9s-style navigation:
- View stack with push/pop semantics
- `:` commands replace root view
- `Enter` pushes detail views
- `Escape`/`q` pops back

## Async Pattern

Message-passing for async:
- UI loop owns all state
- Async tasks send results via channel (`Event::Jira`)
- Views implement `receive_data()` to handle async results
- No shared mutable state across async boundaries

## Environment

- `J9S_JIRA_TOKEN` - Jira API token (required)
- Config at `~/.config/j9s/config.yaml`
