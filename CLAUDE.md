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
- `src/db/` - SQLite connection management
- `src/cache/` - Generic caching layer (see below)

## UI Architecture

```
src/ui/
├── mod.rs           # Main draw function
├── view.rs          # View trait and ViewAction enum
├── components/      # Stateful components with co-located rendering
│   ├── key_result.rs        # KeyResult<T> enum for key handling
│   ├── input.rs             # TextInput - base input component
│   ├── search_input.rs      # SearchInput - wraps TextInput, renders overlay
│   ├── command_input.rs     # CommandInput - wraps TextInput + autocomplete
│   ├── filter_bar.rs        # FilterBar - tab-based filter selection
│   ├── filter_field_picker.rs # FilterFieldPicker - filter field selector
│   └── status_picker.rs     # StatusPicker - status selection overlay
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
- **Components** are reusable input handlers with `KeyResult<T>` pattern
- **renderfns** are stateless - just take data and render, no state

When adding a new view: create in `ui/views/`, implement `View` trait, co-locate rendering.
When adding a new component: create in `ui/components/`, co-locate rendering with the component.

## Component Key Handling with KeyResult<T>

Components use `KeyResult<T>` for consistent key event handling:

```rust
pub enum KeyResult<T> {
  Handled,      // Key consumed, no event for parent
  Event(T),     // Key consumed, here's an event for parent
  NotHandled,   // Key not consumed, try next handler
}
```

Each component defines its own event enum (e.g. `FilterBarEvent`, `SearchEvent`).

**Views use or_else chains** to delegate keys through the component stack:

```rust
fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
  self
    .handle_overlays(key)
    .or_else(|| self.handle_navigation(key))
    .or_else(|| self.handle_toggles(key))
    .or_else(|| self.handle_actions(key))
    .unwrap_or(ViewAction::None)
}

fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
  match self.search.handle_key(key) {
    KeyResult::Handled => return Some(ViewAction::None),
    KeyResult::Event(SearchEvent::Submitted(q)) => { /* use q */ }
    KeyResult::Event(SearchEvent::Cancelled) => return Some(ViewAction::None),
    KeyResult::NotHandled => {}
  }
  // ... more components
  None
}
```

**When adding a new component:**
1. Define an event enum: `pub enum FooEvent { Selected(T), Cancelled }`
2. Implement `handle_key(&mut self, key: KeyEvent) -> KeyResult<FooEvent>`
3. Component returns `NotHandled` when inactive, handles keys when active
4. Parent view matches on the result in its `handle_overlays` method

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

## Caching with CacheLayer

The `src/cache/` module provides transparent caching with offline support:

```
src/cache/
├── mod.rs      # Module exports
├── traits.rs   # Cacheable trait, CacheResult<T>, CacheSource
├── layer.rs    # CacheLayer<S> - orchestrates caching logic
└── storage.rs  # CacheStorage trait, SqliteStorage, NoopStorage
```

**Core concepts:**

- **Cacheable trait** - Entities must implement `cache_key()`, `updated_at()`, `entity_type()`
- **CacheLayer<S>** - Wraps a storage backend, provides fetch methods
- **CacheStorage trait** - Abstraction for storage backends (SQLite, Noop)
- **CacheResult<T>** - Contains data + source (Network, CacheFresh, CacheStale, Offline)

**Fetch strategies:**

```rust
// Simple list fetch (cache-first, offline fallback)
cache.fetch_list("boards:PROJECT", || async { jira.get_boards().await }).await

// Incremental fetch (only fetches entities updated since last sync)
cache.fetch_incremental("issues:jql", |since| async move {
    jira.search_issues_since(jql, since).await
}).await

// Single entity fetch
cache.fetch_one("issue:PROJ-123", || async { jira.get_issue(key).await }).await
```

**Key behaviors:**

- Cache-first: returns fresh cache immediately, avoids network
- Stale-while-revalidate: returns stale cache while fetching in background
- Offline mode: on network failure, returns stale cache with `CacheSource::Offline`
- Incremental sync: uses `updated_at > max_cached_updated_at` for efficient updates

## Environment

- `J9S_JIRA_TOKEN` - Jira API token (required)
- Config at `~/.config/j9s/config.yaml`
