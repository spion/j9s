# QueryClient Future Plan

## Current State

Views own individual `Query<T>` instances. Each view:
- Creates its own query with a fetcher closure
- Calls `poll()` in `tick()` to check for results
- Manages its own loading/success/error state

This works but doesn't enable caching or deduplication.

## Proposed: Centralized QueryClient

A single `QueryClient` shared via `Arc<QueryClient>`. Uses DashMap internally so all methods take `&self` - no Mutex/RefCell needed at the API level:

```rust
pub struct QueryClient {
    cache: DashMap<QueryKey, CachedEntry>,
    in_flight: DashMap<QueryKey, InFlightQuery>,
    stale_time: Duration,
    cache_time: Duration,
}

impl QueryClient {
    // All methods take &self - DashMap handles synchronization internally
    fn get<T>(&self, key: &QueryKey, fetcher: impl Fetcher<T>) -> QueryResult<T>;
    fn refetch(&self, key: &QueryKey);
    fn invalidate(&self, key: &QueryKey);
    fn poll_all(&self);  // Called once per tick
}
```

### Query Keys

Typed keys for cache lookup:

```rust
enum QueryKey {
    Issues { jql: String },
    Issue { key: String },
    Boards,
    BoardData { board_id: u64 },
}
```

### Benefits

1. **Caching** - Same query key returns cached data
2. **Deduplication** - Multiple views requesting same data → one fetch
3. **Stale-while-revalidate** - Return stale data immediately, refetch in background
4. **Single tick point** - `query_client.poll_all()` instead of per-view polling
5. **Invalidation** - After mutations, invalidate affected keys

### View Changes

Views hold `Arc<QueryClient>` and a query key:

```rust
struct IssueListView {
    query_client: Arc<QueryClient>,
    query_key: QueryKey,
    // ... UI state
}

impl IssueListView {
    fn new(project: String, qc: Arc<QueryClient>) -> Self {
        let query_key = QueryKey::Issues { jql: format!("project = {}", project) };
        // Optionally start fetch immediately
        qc.fetch(&query_key, fetcher);
        Self { query_client: qc, query_key, ... }
    }
}

impl View for IssueListView {
    fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
        match key.code {
            KeyCode::Char('r') => {
                self.query_client.refetch(&self.query_key);  // Direct call, no action needed
                ViewAction::None
            }
            // ...
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect) {
        match self.query_client.get(&self.query_key) {
            QueryResult::Loading => ...,
            QueryResult::Success(data) => ...,
            QueryResult::Error(e) => ...,
        }
    }
}
```

No View trait changes needed - views call QueryClient methods directly.

### App Changes

```rust
struct App {
    query_client: Arc<QueryClient>,
    // ...
}

impl App {
    fn new() -> Self {
        let query_client = Arc::new(QueryClient::new());
        let view = IssueListView::new(project, query_client.clone());
        // ...
    }

    fn handle_tick(&mut self) {
        self.query_client.poll_all();  // &self method
    }
}
```

## Implementation Steps

1. Add `dashmap` dependency
2. Design `QueryKey` enum with all data types
3. Implement `QueryClient` with DashMap-based caching
4. Add `poll_all()` that checks all in-flight queries
5. Migrate views one by one to use `Arc<QueryClient>`
6. Remove per-view `Query<T>` once all views migrated
7. Add stale time / cache time configuration
8. Add invalidation API for future mutations

## Open Questions

- How to handle generic types in cache? (likely `Box<dyn Any>` with downcast, or separate typed caches)
- How to handle query garbage collection when views are popped?
- Should fetchers be registered once, or passed on each get()?

## Dependencies

- `dashmap` crate for lock-free concurrent HashMap
