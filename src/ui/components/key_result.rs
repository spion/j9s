/// Generic result type for component key handling.
///
/// This enum standardizes how components communicate key handling results
/// to their parent views, replacing component-specific result enums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyResult<T> {
  /// Key was consumed, no event for parent to handle
  Handled,
  /// Key was consumed, here's an event for parent to process
  Event(T),
  /// Key was not consumed, parent should try next handler
  NotHandled,
}
