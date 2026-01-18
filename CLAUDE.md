# j9s Development Notes

## Code Style

- Rust formatting: 100 columns, 2-space indentation (see `rustfmt.toml`)
- Run `cargo fmt` before committing

## Project Structure

- `src/app.rs` - Main app state and event loop
- `src/event.rs` - Event types (Key, Tick)
- `src/query.rs` - Query<T> for async data fetching
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
│   ├── issue_detail.rs  # IssueDetailView
│   ├── board_list.rs    # BoardListView
│   └── board.rs         # BoardView (kanban/list)
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

## Data Fetching with Query<T>

Views own their data loading via `Query<T>` (inspired by TanStack Query):

```rust
// View creates and owns its query
let mut query = Query::new(move || {
    let jira = jira.clone();
    async move { jira.search_issues(&jql).await.map_err(|e| e.to_string()) }
});
query.fetch();  // Start loading immediately

// In tick() - poll for results
fn tick(&mut self) {
    self.query.poll();
}

// In render() - use query state
match self.query.state() {
    QueryState::Loading => render_spinner(),
    QueryState::Success(data) => render_data(data),
    QueryState::Error(e) => render_error(e),
    QueryState::Idle => {}
}
```

**Key points:**
- Views take `JiraClient` in constructor and create their own queries
- `Query<T>` handles loading/success/error states internally
- App calls `tick()` on all views each tick to poll queries
- No event routing needed - views are self-contained
- Testable: create view with mock fetcher, call methods, check state

## Environment

- `J9S_JIRA_TOKEN` - Jira API token (required)
- Config at `~/.config/j9s/config.yaml`
