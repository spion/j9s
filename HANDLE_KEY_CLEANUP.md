# Handle Key Cleanup Plan

## Problem

The `handle_key` dispatch in views is ad-hoc and inconsistent:

```rust
// BoardView::handle_key - current pattern
fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
  self.error_message = None;  // side effect before dispatch

  // Pattern 1: is_active() check + match on result enum
  if self.status_picker.is_active() {
    match self.status_picker.handle_key(key) {
      StatusPickerResult::Active => return ViewAction::None,
      StatusPickerResult::Selected(id) => { /* do work */ return ViewAction::None; }
      StatusPickerResult::Cancelled => { /* cleanup */ return ViewAction::None; }
      StatusPickerResult::NotHandled => {}
    }
  }

  // Pattern 2: no is_active() check, match on result enum
  match self.search.handle_key(key) {
    SearchResult::Active => return ViewAction::None,
    SearchResult::Submitted(query) => { /* do work */ return ViewAction::None; }
    SearchResult::Cancelled => return ViewAction::None,
    SearchResult::NotHandled => {}
  }

  // Pattern 3: big match on key codes
  match key.code {
    KeyCode::Char('j') => { /* nav */ }
    KeyCode::Char('s') => { /* toggle */ }
    KeyCode::Enter => { return ViewAction::Push(...); }
    // ... 50+ lines
  }
  ViewAction::None
}
```

Issues:
1. Different check patterns (`is_active()` guard vs implicit)
2. Different result enum shapes per component
3. Ordering implicit in code structure
4. Large match blocks mixing navigation, toggles, and actions

## Proposed Pattern

Uniform dispatch using `Option<ViewAction>`:

```rust
fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
  self.handle_overlays(key)
    .or_else(|| self.handle_navigation(key))
    .or_else(|| self.handle_toggles(key))
    .or_else(|| self.handle_actions(key))
    .unwrap_or(ViewAction::None)
}
```

Rules:
- Every handler returns `Option<ViewAction>`
- `Some(action)` = "I handled this key"
- `None` = "Not my key, pass to next handler"
- Handlers can do side effects before returning
- Order is explicit in the chain

## Component Changes

### Option A: Components return `Option<ComponentEvent>`

Components define their own event type for things the parent needs to act on:

```rust
// StatusPicker
pub enum StatusPickerEvent {
  Selected(String),
  Cancelled,
}

impl StatusPicker {
  // Returns None if not active or key not handled
  // Returns Some(event) if something happened parent needs to know
  pub fn handle_key(&mut self, key: KeyEvent) -> Option<StatusPickerEvent> {
    if !self.active { return None; }

    match key.code {
      KeyCode::Enter => {
        let id = self.statuses[self.selected].id.clone();
        self.hide();
        Some(StatusPickerEvent::Selected(id))
      }
      KeyCode::Esc => {
        self.hide();
        Some(StatusPickerEvent::Cancelled)
      }
      KeyCode::Char('j') | KeyCode::Down => {
        self.selected = (self.selected + 1) % self.statuses.len();
        Some(StatusPickerEvent::Cancelled) // Handled, but parent doesn't need to act
        // Alternative: Return a sentinel like StatusPickerEvent::Handled
      }
      _ => None,
    }
  }
}
```

Problem: "I handled the key but parent doesn't need to do anything" is awkward.
Either add a `Handled` variant or use a different return type.

### Option B: Components return `Option<Option<ComponentEvent>>`

```rust
// Some(Some(event)) = handled, here's what happened
// Some(None) = handled, nothing for parent
// None = not handled
```

This is awkward to use.

### Option C: Components return `KeyResult<T>`

```rust
pub enum KeyResult<T> {
  Handled,           // Key consumed, no event for parent
  Event(T),          // Key consumed, here's an event for parent
  NotHandled,        // Key not consumed
}

impl StatusPicker {
  pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult<StatusPickerEvent> {
    if !self.active { return KeyResult::NotHandled; }

    match key.code {
      KeyCode::Enter => {
        let id = self.statuses[self.selected].id.clone();
        self.hide();
        KeyResult::Event(StatusPickerEvent::Selected(id))
      }
      KeyCode::Esc => {
        self.hide();
        KeyResult::Event(StatusPickerEvent::Cancelled)
      }
      KeyCode::Char('j') | KeyCode::Down => {
        self.selected = (self.selected + 1) % self.statuses.len();
        KeyResult::Handled
      }
      _ => KeyResult::NotHandled,
    }
  }
}
```

View usage:
```rust
fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
  match self.status_picker.handle_key(key) {
    KeyResult::Event(StatusPickerEvent::Selected(id)) => {
      self.update_issue_status(&id);
      Some(ViewAction::None)
    }
    KeyResult::Event(StatusPickerEvent::Cancelled) => Some(ViewAction::None),
    KeyResult::Handled => Some(ViewAction::None),
    KeyResult::NotHandled => {}
  }

  // Continue to search...
  match self.search.handle_key(key) {
    // ...
  }

  None
}
```

### Option D: Keep current result enums, just make dispatch consistent

Don't change component interfaces. Just organize view's handle_key better:

```rust
fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
  self.handle_overlays(key)
    .or_else(|| self.handle_navigation(key))
    .or_else(|| self.handle_toggles(key))
    .or_else(|| self.handle_actions(key))
    .unwrap_or(ViewAction::None)
}

fn handle_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
  // Status picker
  if self.status_picker.is_active() {
    match self.status_picker.handle_key(key) {
      StatusPickerResult::Active => return Some(ViewAction::None),
      StatusPickerResult::Selected(id) => {
        if let Some(issue_key) = self.pending_issue_key.take() {
          self.update_issue_status(&issue_key, &id);
        }
        return Some(ViewAction::None);
      }
      StatusPickerResult::Cancelled => {
        self.pending_issue_key = None;
        return Some(ViewAction::None);
      }
      StatusPickerResult::NotHandled => {}
    }
  }

  // Search
  match self.search.handle_key(key) {
    SearchResult::Active => return Some(ViewAction::None),
    SearchResult::Submitted(query) => {
      // TODO: apply filter
      return Some(ViewAction::None);
    }
    SearchResult::Cancelled => return Some(ViewAction::None),
    SearchResult::NotHandled => {}
  }

  None
}

fn handle_navigation(&mut self, key: KeyEvent) -> Option<ViewAction> {
  let handled = match key.code {
    KeyCode::Char('j') | KeyCode::Down => {
      if self.swimlane_mode {
        self.navigate_swimlane(1, false);
      } else {
        self.navigate_list(1);
      }
      true
    }
    KeyCode::Char('k') | KeyCode::Up => {
      if self.swimlane_mode {
        self.navigate_swimlane(-1, false);
      } else {
        self.navigate_list(-1);
      }
      true
    }
    // ... other nav keys
    _ => false,
  };
  handled.then_some(ViewAction::None)
}

fn handle_toggles(&mut self, key: KeyEvent) -> Option<ViewAction> {
  match key.code {
    KeyCode::Char('s') => {
      self.swimlane_mode = !self.swimlane_mode;
      self.reset_selection();
      Some(ViewAction::None)
    }
    KeyCode::Char('f') if !self.quick_filters().is_empty() => {
      self.filter_bar_active = !self.filter_bar_active;
      Some(ViewAction::None)
    }
    _ => None,
  }
}

fn handle_actions(&mut self, key: KeyEvent) -> Option<ViewAction> {
  match key.code {
    KeyCode::Char('r') => {
      self.query.refetch();
      Some(ViewAction::None)
    }
    KeyCode::Enter => {
      self.selected_issue().map(|issue| {
        ViewAction::Push(Box::new(IssueDetailView::new(
          issue.key.clone(),
          self.jira.clone(),
        )))
      })
    }
    KeyCode::Char('q') | KeyCode::Esc => Some(ViewAction::Pop),
    _ => None,
  }
}
```

## Recommendation

Start with **Option D** (organize dispatch, keep component interfaces).

Reasons:
1. Smallest change, lowest risk
2. Addresses the main complaint (ad-hoc dispatch)
3. Components stay simple
4. Can evolve to Option C later if needed

Later, if we want reusable components across views, consider Option C (`KeyResult<T>`).

## Migration Steps

1. [ ] Add helper methods to BoardView: `handle_overlays`, `handle_navigation`, `handle_toggles`, `handle_actions`
2. [ ] Refactor `handle_key` to use the or_else chain
3. [ ] Apply same pattern to other views (IssueListView, BoardListView, IssueDetailView)
4. [ ] Consider extracting `KeyResult<T>` if pattern proves useful

## Open Questions

1. Should we remove the `is_active()` check pattern? SearchInput doesn't use it, StatusPicker does.
   - Option: Have components always return NotHandled when inactive (internally check)

2. Should navigation be a reusable component (`ListNavigator`, `GridNavigator`)?
   - Deferred: Start with method extraction, extract components if duplication appears

3. Should the error message clearing (`self.error_message = None`) be part of the chain or happen first?
   - Probably first, as a side effect before dispatch (current behavior)
